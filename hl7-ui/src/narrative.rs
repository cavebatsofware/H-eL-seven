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

//! Generated narrative: generic sections derived from the converted JSON and
//! the loaded definitions - works for any message structure, not just ADT.

use crate::convert::Converted;
use hl7_engine::defs::Definitions;
use serde_json::Value;

pub struct Section {
    pub title: String,
    pub sub: String,
    pub rows: Vec<(String, String)>,
}

const MAX_OCCURRENCES: usize = 5;
const MAX_VALUE_LEN: usize = 120;

pub fn build(conv: &Converted, defs: Option<&Definitions>) -> Vec<Section> {
    let mut sections = Vec::new();

    // Message header from _meta (+ MSH processing id).
    let processing = walk(&conv.message, &["MSH", "processingId", "processingId"])
        .map(summarize_leaf)
        .unwrap_or_else(|| "-".to_string());
    sections.push(Section {
        title: "Message header".to_string(),
        sub: format!("MSH · {}", conv.meta.control_id),
        rows: vec![
            ("Message type".to_string(), conv.meta.msg_type.clone()),
            ("HL7 version".to_string(), conv.meta.hl7_version.clone()),
            ("Sending".to_string(), conv.meta.from.clone()),
            ("Receiving".to_string(), conv.meta.to.clone()),
            ("Processing ID".to_string(), processing),
            ("Timestamp".to_string(), dash(&conv.meta.received)),
        ],
    });

    // One section per segment code, in wire order, skipping MSH.
    for seg in conv.segments.iter().filter(|s| s.code != "MSH") {
        let mut rows = Vec::new();
        let occurrences: Vec<&Value> = collect_occurrences(conv, &seg.code);
        for (i, occ) in occurrences.iter().take(MAX_OCCURRENCES).enumerate() {
            let prefix = if occurrences.len() > 1 {
                format!("#{} · ", i + 1)
            } else {
                String::new()
            };
            if let Value::Object(fields) = occ {
                for (key, value) in fields {
                    if matches!(value, Value::Null) {
                        continue;
                    }
                    rows.push((format!("{prefix}{}", humanize(key)), summarize(value)));
                }
            }
        }
        if occurrences.len() > MAX_OCCURRENCES {
            rows.push((
                "…".to_string(),
                format!(
                    "and {} more occurrence(s)",
                    occurrences.len() - MAX_OCCURRENCES
                ),
            ));
        }
        if rows.is_empty() {
            continue;
        }
        sections.push(Section {
            title: seg.label.clone(),
            sub: format!("{} · {} occurrence(s)", seg.code, seg.count),
            rows,
        });
    }

    // Conversion notes.
    let count_of = |sev: &str| conv.issues.iter().filter(|i| i.severity == sev).count();
    let unmapped = if conv.unexpected.is_empty() {
        "none".to_string()
    } else {
        format!(
            "{} ({})",
            conv.unexpected.len(),
            conv.unexpected
                .iter()
                .map(|u| u.segment.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    sections.push(Section {
        title: "Conversion notes".to_string(),
        sub: format!(
            "mapping report · defs v{}",
            defs.map(|d| d.version.as_str()).unwrap_or("?")
        ),
        rows: vec![
            ("Segments mapped".to_string(), conv.seg_count.to_string()),
            (
                "Issues".to_string(),
                format!(
                    "{} error, {} warning, {} info",
                    count_of("error"),
                    count_of("warning"),
                    count_of("info")
                ),
            ),
            ("Unmapped segments".to_string(), unmapped),
        ],
    });

    sections
}

/// All occurrences of a segment code anywhere in the message, via the
/// already-computed node ids ("root.patientResult.0.OBX.1" -> JSON path).
fn collect_occurrences<'m>(conv: &'m Converted, code: &str) -> Vec<&'m Value> {
    let mut out = Vec::new();
    for occ in 1.. {
        match conv.seg_occurrences.get(&(code.to_string(), occ)) {
            Some(node_id) => {
                if let Some(v) = by_node_id(&conv.message, node_id) {
                    out.push(v);
                }
            }
            None => break,
        }
    }
    out
}

fn by_node_id<'m>(message: &'m Value, node_id: &str) -> Option<&'m Value> {
    let mut cur = message;
    for part in node_id.split('.').skip(1) {
        cur = match cur {
            Value::Array(a) => a.get(part.parse::<usize>().ok()?)?,
            Value::Object(o) => o.get(part)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn walk<'m>(message: &'m Value, path: &[&str]) -> Option<&'m Value> {
    let mut cur = message;
    for part in path {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn dash(s: &str) -> String {
    if s.is_empty() {
        "-".to_string()
    } else {
        s.to_string()
    }
}

/// "patientIdentifierList" -> "Patient identifier list".
fn humanize(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (i, c) in key.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_ascii_uppercase() {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn summarize_leaf(v: &Value) -> String {
    match v {
        Value::String(s) => s.replace('T', " "),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

/// Compact display of any value: leaves verbatim, objects as their first few
/// leaf values, arrays as joined element summaries; everything truncated.
fn summarize(v: &Value) -> String {
    let text = match v {
        Value::Object(o) => {
            let leaves: Vec<String> = o
                .values()
                .filter(|v| !v.is_null())
                .take(4)
                .map(summarize)
                .collect();
            leaves.join(" · ")
        }
        Value::Array(a) => {
            let items: Vec<String> = a.iter().map(summarize).collect();
            items.join("; ")
        }
        Value::String(s) => s.replace('T', " "),
        other => other.to_string(),
    };
    if text.chars().count() > MAX_VALUE_LEN {
        let truncated: String = text.chars().take(MAX_VALUE_LEN).collect();
        format!("{truncated}…")
    } else {
        text
    }
}
