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

//! Generic segment ↔ JSON-node mapping. In the emitted document segment
//! values keep their uppercase 3-char id as key while groups are camelCase
//! (see hl7-engine json.rs), so segment nodes can be found by key shape at
//! any nesting depth - no hardcoded IN1->insurance style table.

use hl7_engine::defs::Definitions;
use serde_json::Value;

/// A key is a segment iff the definitions know it or it looks like a
/// segment id (covers Z-segments the defs don't list). Group and field
/// keys are camelCase and can never match.
pub fn is_segment_key(key: &str, defs: Option<&Definitions>) -> bool {
    if let Some(defs) = defs {
        if defs.segments.contains_key(key) {
            return true;
        }
    }
    key.len() == 3
        && key.as_bytes()[0].is_ascii_uppercase()
        && key
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// DFS the message for segment occurrences -> (code, json node id) pairs.
/// One object value = one occurrence; an array = one per element. Group
/// arrays are visited in element order, so per-code occurrence order matches
/// wire order in practice (the one exception - a code split across sibling
/// group keys iterated alphabetically - is acceptable for a viewer).
pub fn map_segments(message: &Value, defs: Option<&Definitions>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    descend(message, "root", defs, &mut out);
    out
}

fn descend(value: &Value, id: &str, defs: Option<&Definitions>, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(o) => {
            for (k, v) in o {
                let child_id = format!("{id}.{k}");
                if is_segment_key(k, defs) {
                    match v {
                        Value::Array(a) => {
                            for i in 0..a.len() {
                                out.push((k.clone(), format!("{child_id}.{i}")));
                            }
                        }
                        _ => out.push((k.clone(), child_id)),
                    }
                    // Field data cannot contain segment keys - don't descend.
                } else {
                    descend(v, &child_id, defs, out);
                }
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                descend(v, &format!("{id}.{i}"), defs, out);
            }
        }
        _ => {}
    }
}

/// Node id + all its ancestor ids ("root.patientResult.0.OBX.1" ->
/// ["root", "root.patientResult", …, itself]) - everything that must be
/// expanded so the node is visible.
pub fn with_ancestors(node_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut end = 0;
    let bytes = node_id.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'.' {
            out.push(node_id[..i].to_string());
        }
        end = i + 1;
    }
    out.push(node_id[..end].to_string());
    out
}
