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

//! Conversion model: run the engine over raw text and derive everything the
//! explorer needs (raw lines, parsed issues, segment entries, JSON nodes,
//! segment↔node mapping). Built once per conversion, shared as Rc.

use crate::jsontree::{self, JsonNode};
use crate::segmap;
use hl7_engine::defs::Definitions;
use serde_json::Value;
use std::collections::HashMap;

pub struct MetaView {
    pub event: String,
    pub control_id: String,
    pub hl7_version: String,
    pub defs_version: String,
    pub from: String,
    pub to: String,
    pub received: String,
    pub msg_type: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LineMark {
    None,
    Issue,
    Unmapped,
}

pub struct RawLine {
    pub n: usize,
    pub code: String,
    pub occ: usize,
    pub rest: String,
    pub mark: LineMark,
    /// Tooltip: the issue message, if any.
    pub title: String,
}

pub struct ParsedIssue {
    pub severity: String,
    pub location: String,
    pub message: String,
    pub seg: String,
    pub occ: usize,
}

pub struct UnexpectedView {
    pub segment: String,
    pub position: u64,
    pub detail: String,
}

pub struct SegEntry {
    pub code: String,
    pub label: String,
    pub count: usize,
    pub flagged: bool,
}

pub struct Converted {
    pub raw_lines: Vec<RawLine>,
    pub message: Value,
    pub meta: MetaView,
    pub issues: Vec<ParsedIssue>,
    pub unexpected: Vec<UnexpectedView>,
    pub segments: Vec<SegEntry>,
    pub nodes: Vec<JsonNode>,
    /// (code, occurrence starting at 1) -> JSON node id.
    pub seg_occurrences: HashMap<(String, usize), String>,
    pub seg_count: usize,
}

/// Identity equality: a conversion is immutable once built, so two are equal
/// iff they are the same allocation. Lets Rc<Converted> flow through props
/// and memos without deep comparison.
impl PartialEq for Converted {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

/// "ROL[2]-12[1].5" -> ("ROL", 2). No-bracket forms get occurrence 1.
pub fn parse_location(location: &str) -> (String, usize) {
    let seg_end = location
        .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit()))
        .unwrap_or(location.len());
    let seg = location[..seg_end].to_string();
    let occ = location[seg_end..]
        .strip_prefix('[')
        .and_then(|rest| rest.split(']').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(1);
    (seg, occ)
}

/// MSH-12 from raw text without a full parse: field separator is byte 3;
/// MSH-2 is the encoding chars, so version is the 11th separated token
/// after "MSH|".
pub fn sniff_msh12(text: &str) -> Option<String> {
    let line = text.trim_start_matches(['\u{b}', '\u{1c}', '\r', '\n', ' ']);
    let rest = line.strip_prefix("MSH")?;
    let sep = rest.chars().next()?;
    if sep.is_alphanumeric() {
        return None;
    }
    let first_line = rest.split(['\r', '\n']).next()?;
    // Tokens after MSH-1: [0]=MSH-2 … [10]=MSH-12.
    let field = first_line[sep.len_utf8()..].split(sep).nth(10)?;
    let version: String = field
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!version.is_empty()).then_some(version)
}

/// Split into segment lines the way the CLI does: a new message line per
/// CR/LF, keeping non-empty ones.
fn split_lines(text: &str) -> Vec<&str> {
    text.split(['\r', '\n'])
        .map(|l| l.trim_matches(['\u{b}', '\u{1c}']))
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn build(text: &str, doc: Value, defs: Option<&Definitions>) -> Converted {
    let message = doc.get("message").cloned().unwrap_or(Value::Null);
    let meta_v = &doc["_meta"];
    let s = |k: &str| meta_v[k].as_str().unwrap_or("").to_string();

    let meta = MetaView {
        event: s("event"),
        control_id: s("messageControlId"),
        hl7_version: s("hl7Version"),
        defs_version: s("definitionsVersion"),
        from: format!("{} · {}", s("sendingApplication"), s("sendingFacility")),
        to: format!("{} · {}", s("receivingApplication"), s("receivingFacility")),
        received: s("messageDateTime").replace('T', " "),
        msg_type: format!(
            "{}^{} ({})",
            s("messageType"),
            s("triggerEvent"),
            s("messageStructure")
        ),
    };

    let issues: Vec<ParsedIssue> = doc["issues"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|i| {
                    let location = i["location"].as_str().unwrap_or("").to_string();
                    let (seg, occ) = parse_location(&location);
                    ParsedIssue {
                        severity: i["severity"].as_str().unwrap_or("info").to_string(),
                        message: i["message"].as_str().unwrap_or("").to_string(),
                        location,
                        seg,
                        occ,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let unexpected: Vec<UnexpectedView> = doc["unexpected"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|u| UnexpectedView {
                    segment: u["segment"].as_str().unwrap_or("").to_string(),
                    position: u["position"].as_u64().unwrap_or(0),
                    detail: format!(
                        "fields: {}",
                        u["fields"]
                            .as_array()
                            .map(|f| {
                                f.iter()
                                    .map(|v| match v {
                                        Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" · ")
                            })
                            .unwrap_or_default()
                    ),
                })
                .collect()
        })
        .unwrap_or_default();

    let unmapped_codes: Vec<&str> = unexpected.iter().map(|u| u.segment.as_str()).collect();
    let issue_by_line: HashMap<(String, usize), &ParsedIssue> = issues
        .iter()
        .filter(|i| !unmapped_codes.contains(&i.seg.as_str()))
        .map(|i| ((i.seg.clone(), i.occ), i))
        .collect();

    // Raw lines with occurrence counters and issue/unmapped marks.
    let mut counters: HashMap<String, usize> = HashMap::new();
    let raw_lines: Vec<RawLine> = split_lines(text)
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let code_len = line
                .find(|c: char| !(c.is_ascii_alphanumeric()))
                .unwrap_or(line.len().min(3));
            let code = line[..code_len].to_string();
            let occ = counters
                .entry(code.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            let occ = *occ;
            let unmapped = unmapped_codes.contains(&code.as_str());
            let issue = issue_by_line.get(&(code.clone(), occ));
            let (mark, title) = if unmapped {
                (
                    LineMark::Unmapped,
                    "Unmapped segment - preserved without a definition".to_string(),
                )
            } else if let Some(iss) = issue {
                (LineMark::Issue, iss.message.clone())
            } else {
                (LineMark::None, String::new())
            };
            RawLine {
                n: i + 1,
                rest: line[code.len()..].to_string(),
                code,
                occ,
                mark,
                title,
            }
        })
        .collect();

    // Segment -> JSON node mapping (generic DFS).
    let occurrences = segmap::map_segments(&message, defs);
    let mut seg_occurrences: HashMap<(String, usize), String> = HashMap::new();
    let mut per_code: HashMap<&str, usize> = HashMap::new();
    for (code, node_id) in &occurrences {
        let n = per_code
            .entry(code.as_str())
            .and_modify(|c| *c += 1)
            .or_insert(1);
        seg_occurrences.insert((code.clone(), *n), node_id.clone());
    }

    // Rail entries in wire order (first appearance in raw lines), mapped
    // segments only; unmapped ones get their own rail section.
    let flagged: Vec<&str> = issue_by_line.keys().map(|(s, _)| s.as_str()).collect();
    let mut segments: Vec<SegEntry> = Vec::new();
    for line in &raw_lines {
        if segments.iter().any(|s| s.code == line.code) {
            continue;
        }
        if unmapped_codes.contains(&line.code.as_str()) {
            continue;
        }
        let label = defs
            .and_then(|d| d.segments.get(&line.code))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Segment".to_string());
        segments.push(SegEntry {
            label,
            count: raw_lines.iter().filter(|l| l.code == line.code).count(),
            flagged: flagged.contains(&line.code.as_str()),
            code: line.code.clone(),
        });
    }

    let seg_count = occurrences.len();
    let nodes = jsontree::flatten("message", &message);

    Converted {
        raw_lines,
        message,
        meta,
        issues,
        unexpected,
        segments,
        nodes,
        seg_occurrences,
        seg_count,
    }
}
