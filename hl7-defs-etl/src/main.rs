// H-eL-seven - a schema-aware HL7 v2 to JSON translator
// Copyright (C) 2026 CavebatSoftware LLC - Grant DeFayette
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Scrapes an HL7-Definition API into one normalized JSON snapshot
//! per HL7 version, written to defs/hl7-<version>.json.
//!
//! Usage: hl7-defs-etl [versions...]   (default: 2.5.1)
//! The API base URL can be overridden via the HL7_DEFS_API_BASE env var.

use hl7_engine::defs::*;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Mutex;

const WORKERS: usize = 8;

#[derive(Deserialize)]
struct RawListItem {
    id: Option<String>,
}

#[derive(Deserialize)]
struct RawStructItem {
    id: Option<String>,
    name: Option<String>,
    usage: Option<String>,
    rpt: Option<String>,
    #[serde(rename = "isGroup", default)]
    is_group: bool,
    segments: Option<Vec<RawStructItem>>,
}

#[derive(Deserialize)]
struct RawEvent {
    #[serde(rename = "msgStructId")]
    msg_struct_id: Option<String>,
    #[serde(rename = "eventDesc")]
    event_desc: Option<String>,
    segments: Option<Vec<RawStructItem>>,
}

#[derive(Deserialize)]
struct RawField {
    #[serde(rename = "dataType")]
    data_type: Option<String>,
    usage: Option<String>,
    rpt: Option<String>,
    length: Option<i64>,
    #[serde(rename = "tableId")]
    table_id: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawSegment {
    #[serde(rename = "longName")]
    long_name: Option<String>,
    fields: Option<Vec<RawField>>,
}

#[derive(Deserialize)]
struct RawDataType {
    name: Option<String>,
    fields: Option<Vec<RawField>>,
}

#[derive(Deserialize)]
struct RawTableEntry {
    value: Option<serde_json::Value>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct RawTable {
    name: Option<String>,
    #[serde(rename = "tableType")]
    table_type: Option<String>,
    entries: Option<Vec<RawTableEntry>>,
}

fn get_json(agent: &ureq::Agent, url: &str) -> Result<serde_json::Value, String> {
    let mut last_err = String::new();
    for attempt in 0..4u64 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }
        match agent.get(url).call() {
            Ok(resp) => return resp.into_json().map_err(|e| format!("{url}: {e}")),
            Err(e) => last_err = format!("{url}: {e}"),
        }
    }
    Err(last_err)
}

fn list_ids(agent: &ureq::Agent, base: &str, kind: &str) -> Vec<String> {
    let val = get_json(agent, &format!("{base}/{kind}"))
        .unwrap_or_else(|e| panic!("listing {kind} failed: {e}"));
    let items: Vec<RawListItem> =
        serde_json::from_value(val).unwrap_or_else(|e| panic!("bad {kind} list: {e}"));
    items.into_iter().filter_map(|i| i.id).collect()
}

/// Fetch every /{kind}/{id} detail with a small worker pool. Failures are
/// reported and skipped rather than aborting the whole scrape.
fn fetch_details(base: &str, kind: &str, ids: &[String]) -> BTreeMap<String, serde_json::Value> {
    let queue = Mutex::new(ids.to_vec());
    let results = Mutex::new(BTreeMap::new());
    let done = std::sync::atomic::AtomicUsize::new(0);
    let total = ids.len();

    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| {
                let agent = ureq::AgentBuilder::new()
                    .timeout(std::time::Duration::from_secs(30))
                    .build();
                loop {
                    let id = match queue.lock().unwrap().pop() {
                        Some(id) => id,
                        None => break,
                    };
                    match get_json(&agent, &format!("{base}/{kind}/{id}")) {
                        Ok(val) => {
                            results.lock().unwrap().insert(id, val);
                        }
                        Err(e) => eprintln!("WARN: skipping {kind}/{id}: {e}"),
                    }
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if n.is_multiple_of(50) || n == total {
                        eprintln!("  {kind}: {n}/{total}");
                    }
                }
            });
        }
    });

    results.into_inner().unwrap()
}

fn norm_structure(items: Vec<RawStructItem>) -> Vec<StructureItem> {
    items
        .into_iter()
        .filter_map(|item| {
            let usage = item.usage.unwrap_or_else(|| "O".into());
            let rpt = item.rpt.unwrap_or_else(|| "1".into());
            if item.is_group {
                let name = item.name?.replace(' ', "_");
                Some(StructureItem::Group {
                    group: name,
                    usage,
                    rpt,
                    children: norm_structure(item.segments.unwrap_or_default()),
                })
            } else {
                Some(StructureItem::Segment {
                    segment: item.id.or(item.name)?,
                    usage,
                    rpt,
                })
            }
        })
        .collect()
}

fn norm_fields(fields: Option<Vec<RawField>>) -> Vec<FieldDef> {
    fields
        .unwrap_or_default()
        .into_iter()
        .map(|f| FieldDef {
            name: f.name.unwrap_or_default(),
            data_type: f.data_type.unwrap_or_default(),
            usage: f.usage.unwrap_or_else(|| "O".into()),
            rpt: f.rpt.unwrap_or_else(|| "1".into()),
            length: f.length.unwrap_or(0),
            table: f.table_id.filter(|t| !t.is_empty()),
        })
        .collect()
}

fn scalar_to_string(v: serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    }
}

fn build_version(api_base: &str, version: &str) -> Definitions {
    let base = format!("{api_base}/HL7v{version}");
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .build();

    let mut defs = Definitions {
        version: version.to_string(),
        events: BTreeMap::new(),
        segments: BTreeMap::new(),
        data_types: BTreeMap::new(),
        tables: BTreeMap::new(),
    };

    for kind in ["TriggerEvents", "Segments", "DataTypes", "Tables"] {
        let ids = list_ids(&agent, &base, kind);
        eprintln!("{kind}: {} ids", ids.len());
        let details = fetch_details(&base, kind, &ids);
        for (id, val) in details {
            match kind {
                "TriggerEvents" => {
                    let raw: RawEvent = match serde_json::from_value(val) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("WARN: bad event {id}: {e}");
                            continue;
                        }
                    };
                    defs.events.insert(
                        id.clone(),
                        MessageEvent {
                            msg_struct_id: raw.msg_struct_id.unwrap_or_else(|| id.clone()),
                            description: raw.event_desc.unwrap_or_default(),
                            structure: norm_structure(raw.segments.unwrap_or_default()),
                        },
                    );
                }
                "Segments" => {
                    let raw: RawSegment = match serde_json::from_value(val) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("WARN: bad segment {id}: {e}");
                            continue;
                        }
                    };
                    defs.segments.insert(
                        id,
                        SegmentDef {
                            name: raw.long_name.unwrap_or_default(),
                            fields: norm_fields(raw.fields),
                        },
                    );
                }
                "DataTypes" => {
                    let raw: RawDataType = match serde_json::from_value(val) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("WARN: bad datatype {id}: {e}");
                            continue;
                        }
                    };
                    defs.data_types.insert(
                        id,
                        DataTypeDef {
                            name: raw.name.unwrap_or_default(),
                            components: norm_fields(raw.fields),
                        },
                    );
                }
                "Tables" => {
                    let raw: RawTable = match serde_json::from_value(val) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("WARN: bad table {id}: {e}");
                            continue;
                        }
                    };
                    let values = raw
                        .entries
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|e| {
                            Some((
                                scalar_to_string(e.value?),
                                e.description.unwrap_or_default(),
                            ))
                        })
                        .collect();
                    defs.tables.insert(
                        id,
                        TableDef {
                            name: raw.name.unwrap_or_default(),
                            table_type: raw.table_type.unwrap_or_else(|| "HL7".into()),
                            values,
                        },
                    );
                }
                _ => unreachable!(),
            }
        }
    }
    defs
}

fn main() {
    let versions: Vec<String> = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            vec!["2.5.1".to_string()]
        } else {
            args
        }
    };

    let api_base = std::env::var("HL7_DEFS_API_BASE")
        .expect("HL7_DEFS_API_BASE must be set to the HL7-Definition API base URL");

    std::fs::create_dir_all("defs").expect("create defs dir");
    for version in versions {
        eprintln!("== HL7 v{version} ==");
        let defs = build_version(&api_base, &version);
        let path = format!("defs/hl7-{version}.json");
        let file = std::fs::File::create(&path).expect("create snapshot file");
        serde_json::to_writer(std::io::BufWriter::new(file), &defs).expect("write snapshot");
        eprintln!(
            "wrote {path}: {} events, {} segments, {} datatypes, {} tables",
            defs.events.len(),
            defs.segments.len(),
            defs.data_types.len(),
            defs.tables.len()
        );
    }
}
