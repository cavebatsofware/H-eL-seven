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

//! Document assembly: bound tree + decoder -> the final JSON document.
//!
//! Shape:
//! {
//!   "_meta":      { version, messageType, triggerEvent, event, messageControlId, sendingApplication, … },
//!   "message":    { MSH: {…}, PATIENT_RESULT-as-"patientResult": [ { PID: {…}, … } ], … },
//!   "unexpected": [ { segment, position, fields } ],   // Z-segments & strays, positional, lossless
//!   "issues":     [ { severity, location, message } ]
//! }

use crate::bind::{BoundEntry, BoundGroup, BoundMessage};
use crate::decode::{to_camel, Decoder};
use crate::defs::Definitions;
use crate::issue::Issue;
use crate::syntax::RawMessage;
use serde_json::{json, Map, Value};

pub fn emit(
    defs: &Definitions,
    msg: &RawMessage<'_>,
    bound: &BoundMessage<'_>,
    issues: &mut Vec<Issue>,
) -> Value {
    let decoder = Decoder {
        defs,
        delims: msg.delims,
    };

    let message = emit_group(&decoder, &bound.root, issues);
    let unexpected: Vec<Value> = bound
        .unexpected
        .iter()
        .map(|u| decoder.segment_positional(u.segment, u.position))
        .collect();

    json!({
        "_meta": meta(defs, msg, bound),
        "message": message,
        "unexpected": unexpected,
        "issues": issues,
    })
}

/// Fallback document when no structure could be resolved: every segment
/// rendered positionally, still typed where segment definitions exist.
pub fn emit_unbound(defs: &Definitions, msg: &RawMessage<'_>, issues: &mut Vec<Issue>) -> Value {
    let decoder = Decoder {
        defs,
        delims: msg.delims,
    };
    let mut counts = std::collections::HashMap::new();
    let segments: Vec<Value> = msg
        .segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let n = counts
                .entry(seg.id)
                .and_modify(|n| *n += 1)
                .or_insert(1usize);
            match defs.segments.get(seg.id) {
                Some(def) => {
                    json!({ "segment": seg.id, "position": i + 1,
                            "fields": decoder.segment(seg, def, *n, issues) })
                }
                None => decoder.segment_positional(seg, i + 1),
            }
        })
        .collect();

    let bound_stub = BoundMessage {
        event: None,
        root: BoundGroup::default(),
        unexpected: Vec::new(),
    };
    json!({
        "_meta": meta(defs, msg, &bound_stub),
        "message": Value::Null,
        "segments": segments,
        "issues": issues,
    })
}

fn meta(defs: &Definitions, msg: &RawMessage<'_>, bound: &BoundMessage<'_>) -> Value {
    let msh = &msg.segments[0];
    let comp = |field: usize, comp: usize| -> Value {
        msh.field(field)
            .and_then(|f| f.repeats.first())
            .and_then(|r| r.components.get(comp - 1))
            .and_then(|c| c.subcomponents.first())
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null)
    };
    json!({
        "hl7Version": comp(12, 1),
        "definitionsVersion": defs.version,
        "event": bound.event,
        "messageType": comp(9, 1),
        "triggerEvent": comp(9, 2),
        "messageStructure": comp(9, 3),
        "messageControlId": comp(10, 1),
        "sendingApplication": comp(3, 1),
        "sendingFacility": comp(4, 1),
        "receivingApplication": comp(5, 1),
        "receivingFacility": comp(6, 1),
        "messageDateTime": msh
            .field(7)
            .map(|f| f.first())
            .filter(|s| !s.is_empty())
            .and_then(crate::decode::datetime_to_iso)
            .map(Value::String)
            .unwrap_or(Value::Null),
    })
}

fn emit_group(decoder: &Decoder<'_>, group: &BoundGroup<'_>, issues: &mut Vec<Issue>) -> Value {
    let mut out = Map::new();
    for entry in &group.entries {
        match entry {
            BoundEntry::Segments {
                id,
                repeats,
                instances,
            } => {
                let def = decoder.defs.segments.get(id);
                let mut values: Vec<Value> = instances
                    .iter()
                    .map(|placed| match def {
                        Some(def) => decoder.segment(placed.segment, def, placed.instance, issues),
                        None => decoder.segment_positional(placed.segment, placed.instance),
                    })
                    .collect();
                let value = if *repeats {
                    Value::Array(values)
                } else if values.len() == 1 {
                    values.pop().unwrap()
                } else {
                    Value::Array(values)
                };
                insert_merging(&mut out, id.clone(), value, *repeats);
            }
            BoundEntry::Groups {
                name,
                repeats,
                instances,
            } => {
                let mut values: Vec<Value> = instances
                    .iter()
                    .map(|g| emit_group(decoder, g, issues))
                    .collect();
                let value = if *repeats {
                    Value::Array(values)
                } else if values.len() == 1 {
                    values.pop().unwrap()
                } else {
                    Value::Array(values)
                };
                insert_merging(&mut out, to_camel(name), value, *repeats);
            }
        }
    }
    Value::Object(out)
}

/// A structure can mention the same segment id at several positions in one
/// group (e.g. NTE after both OBR and OBX slots); merge rather than clobber.
fn insert_merging(out: &mut Map<String, Value>, key: String, value: Value, repeats: bool) {
    match out.remove(&key) {
        None => {
            out.insert(key, value);
        }
        Some(existing) => {
            let mut items = match existing {
                Value::Array(a) if repeats => a,
                other => vec![other],
            };
            match value {
                Value::Array(a) => items.extend(a),
                other => items.push(other),
            }
            out.insert(key, Value::Array(items));
        }
    }
}
