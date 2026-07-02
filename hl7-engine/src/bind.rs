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

//! Structure selection and binding: match the flat segment sequence against
//! the nested group grammar of the message's trigger-event structure.
//!
//! Group definitions cannot recurse, so the grammar is regular; a greedy
//! recursive-descent walk suffices (the HAPI approach). Leniency rules:
//! segments not in the structure at all (Z-segments, foreign segments) are
//! consumed wherever they appear and reported; known segments that appear out
//! of place fall out of the walk and are collected as unexpected trailers.

use crate::defs::{Definitions, MessageEvent, StructureItem};
use crate::issue::{Issue, Severity};
use crate::syntax::{RawMessage, RawSegment};
use std::collections::HashSet;

#[derive(Debug)]
pub struct BoundMessage<'m> {
    /// Resolved trigger-event key in the definitions ("ADT_A01"), if any.
    pub event: Option<String>,
    pub root: BoundGroup<'m>,
    /// Segments that found no home in the structure, in message order.
    pub unexpected: Vec<UnexpectedSegment<'m>>,
}

#[derive(Debug)]
pub struct UnexpectedSegment<'m> {
    /// 1-based position in the original message.
    pub position: usize,
    pub segment: &'m RawSegment<'m>,
}

#[derive(Debug, Default)]
pub struct BoundGroup<'m> {
    pub entries: Vec<BoundEntry<'m>>,
}

#[derive(Debug)]
pub enum BoundEntry<'m> {
    Segments {
        id: String,
        /// Whether the definition allows repeats (drives array vs scalar JSON).
        repeats: bool,
        instances: Vec<PlacedSegment<'m>>,
    },
    Groups {
        name: String,
        repeats: bool,
        instances: Vec<BoundGroup<'m>>,
    },
}

#[derive(Debug)]
pub struct PlacedSegment<'m> {
    /// 1-based instance number among segments with this id, for issue locations.
    pub instance: usize,
    pub segment: &'m RawSegment<'m>,
}

/// Resolve which trigger-event structure a message uses, from MSH-9 and the
/// event registry. Returns the event key plus any resolution issues.
pub fn select_event<'d>(
    defs: &'d Definitions,
    msh: &RawSegment<'_>,
) -> (Option<(String, &'d MessageEvent)>, Vec<Issue>) {
    let mut issues = Vec::new();
    let msg_type = msh
        .field(9)
        .map(|f| {
            f.repeats
                .first()
                .map(|r| {
                    let comp = |i: usize| {
                        r.components
                            .get(i)
                            .and_then(|c| c.subcomponents.first())
                            .copied()
                            .unwrap_or("")
                    };
                    (comp(0), comp(1), comp(2))
                })
                .unwrap_or(("", "", ""))
        })
        .unwrap_or(("", "", ""));
    let (ty, trigger, struct_id) = msg_type;

    // Prefer the specific trigger event (ADT_A08) over the shared structure
    // id in MSH-9.3 (ADT_A01): both resolve to the same grammar, but the
    // event carries more information into _meta.
    let candidates = [
        format!("{ty}_{trigger}"),
        struct_id.to_string(),
        ty.to_string(),
    ];
    for key in candidates.iter().filter(|k| !k.is_empty() && *k != "_") {
        if let Some(event) = defs.events.get(key.as_str()) {
            return (Some((key.clone(), event)), issues);
        }
    }

    issues.push(Issue::new(
        Severity::Error,
        "MSH-9",
        format!(
            "unknown message type: no structure found for {:?} in HL7 v{}",
            format!("{ty}^{trigger}^{struct_id}"),
            defs.version
        ),
    ));
    (None, issues)
}

/// Bind a parsed message against an event structure.
pub fn bind<'m>(
    msg: &'m RawMessage<'m>,
    event_key: &str,
    event: &MessageEvent,
    issues: &mut Vec<Issue>,
) -> BoundMessage<'m> {
    let mut known = HashSet::new();
    collect_segment_ids(&event.structure, &mut known);

    let mut binder = Binder {
        segs: &msg.segments,
        pos: 0,
        known,
        unexpected: Vec::new(),
        issues,
        instance_counts: std::collections::HashMap::new(),
    };

    let root = binder.bind_children(&event.structure, event_key, true);

    // Whatever the walk could not place is an unexpected trailer.
    binder.skip_foreign();
    while binder.pos < binder.segs.len() {
        let seg = &binder.segs[binder.pos];
        binder.issues.push(Issue::new(
            Severity::Warning,
            format!("{}[{}]", seg.id, binder.pos + 1),
            format!(
                "segment {} not expected at this point in {}",
                seg.id, event_key
            ),
        ));
        binder.unexpected.push(UnexpectedSegment {
            position: binder.pos + 1,
            segment: seg,
        });
        binder.pos += 1;
        binder.skip_foreign();
    }

    BoundMessage {
        event: Some(event_key.to_string()),
        root,
        unexpected: binder.unexpected,
    }
}

fn collect_segment_ids<'d>(items: &'d [StructureItem], out: &mut HashSet<&'d str>) {
    for item in items {
        match item {
            StructureItem::Segment { segment, .. } => {
                out.insert(segment.as_str());
            }
            StructureItem::Group { children, .. } => collect_segment_ids(children, out),
        }
    }
}

/// Segment ids that can legally start a group: every leading item's ids up to
/// and including the first required item.
fn first_set<'d>(items: &'d [StructureItem], out: &mut HashSet<&'d str>) {
    for item in items {
        match item {
            StructureItem::Segment { segment, usage, .. } => {
                out.insert(segment.as_str());
                if usage == "R" {
                    return;
                }
            }
            StructureItem::Group {
                children, usage, ..
            } => {
                first_set(children, out);
                if usage == "R" {
                    return;
                }
            }
        }
    }
}

struct Binder<'m, 'i> {
    segs: &'m [RawSegment<'m>],
    pos: usize,
    known: HashSet<&'i str>,
    unexpected: Vec<UnexpectedSegment<'m>>,
    issues: &'i mut Vec<Issue>,
    /// Per-segment-id instance counter for issue locations (PID[2]…).
    instance_counts: std::collections::HashMap<String, usize>,
}

impl<'m, 'i> Binder<'m, 'i> {
    /// Consume any run of segments that appear nowhere in this structure -
    /// Z-segments (info) and foreign standard segments (warning).
    fn skip_foreign(&mut self) {
        while let Some(seg) = self.segs.get(self.pos) {
            if self.known.contains(seg.id) {
                break;
            }
            let is_z = seg.id.starts_with('Z');
            self.issues.push(Issue::new(
                if is_z {
                    Severity::Info
                } else {
                    Severity::Warning
                },
                format!("{}[{}]", seg.id, self.pos + 1),
                if is_z {
                    format!("Z-segment {} preserved without a definition", seg.id)
                } else {
                    format!(
                        "segment {} does not occur in this message structure",
                        seg.id
                    )
                },
            ));
            self.unexpected.push(UnexpectedSegment {
                position: self.pos + 1,
                segment: seg,
            });
            self.pos += 1;
        }
    }

    fn current_id(&self) -> Option<&'m str> {
        self.segs.get(self.pos).map(|s| s.id)
    }

    fn bind_children(
        &mut self,
        items: &[StructureItem],
        path: &str,
        is_root: bool,
    ) -> BoundGroup<'m> {
        let mut group = BoundGroup::default();
        let mut missing: Vec<Issue> = Vec::new();

        for item in items {
            self.skip_foreign();
            match item {
                StructureItem::Segment {
                    segment,
                    usage,
                    rpt,
                } => {
                    let repeats = rpt != "1";
                    let mut instances = Vec::new();
                    while self.current_id() == Some(segment.as_str()) {
                        let n = self
                            .instance_counts
                            .entry(segment.clone())
                            .and_modify(|n| *n += 1)
                            .or_insert(1);
                        instances.push(PlacedSegment {
                            instance: *n,
                            segment: &self.segs[self.pos],
                        });
                        self.pos += 1;
                        if !repeats {
                            break;
                        }
                        self.skip_foreign();
                    }
                    if instances.is_empty() {
                        if usage == "R" {
                            missing.push(Issue::new(
                                Severity::Error,
                                path.to_string(),
                                format!("required segment {segment} is missing"),
                            ));
                        }
                    } else {
                        group.entries.push(BoundEntry::Segments {
                            id: segment.clone(),
                            repeats,
                            instances,
                        });
                    }
                }
                StructureItem::Group {
                    group: name,
                    usage,
                    rpt,
                    children,
                } => {
                    let repeats = rpt != "1";
                    let mut first = HashSet::new();
                    first_set(children, &mut first);

                    let mut instances = Vec::new();
                    loop {
                        self.skip_foreign();
                        match self.current_id() {
                            Some(id) if first.contains(id) => {}
                            _ => break,
                        }
                        let start = self.pos;
                        let child_path = format!("{path}.{name}");
                        let bound = self.bind_children(children, &child_path, false);
                        if self.pos == start {
                            break;
                        }
                        instances.push(bound);
                        if !repeats {
                            break;
                        }
                    }
                    if instances.is_empty() {
                        if usage == "R" {
                            missing.push(Issue::new(
                                Severity::Error,
                                path.to_string(),
                                format!("required group {name} is missing"),
                            ));
                        }
                    } else {
                        group.entries.push(BoundEntry::Groups {
                            name: name.clone(),
                            repeats,
                            instances,
                        });
                    }
                }
            }
        }

        // Missing-required complaints only make sense if this group actually
        // materialized (or is the message root) - otherwise every skipped
        // optional group would report its required children.
        if is_root || !group.entries.is_empty() {
            self.issues.append(&mut missing);
        }
        group
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parse;

    fn defs() -> Definitions {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../defs/hl7-2.5.1.json"
        ))
        .expect("defs snapshot present");
        Definitions::from_json(&bytes).unwrap()
    }

    fn bind_text<'m>(
        defs: &Definitions,
        msg: &'m RawMessage<'m>,
    ) -> (BoundMessage<'m>, Vec<Issue>) {
        let (sel, mut issues) = select_event(defs, &msg.segments[0]);
        let (key, event) = sel.expect("event resolved");
        let bound = bind(msg, &key, event, &mut issues);
        (bound, issues)
    }

    fn entry_ids(group: &BoundGroup) -> Vec<String> {
        group
            .entries
            .iter()
            .map(|e| match e {
                BoundEntry::Segments { id, .. } => id.clone(),
                BoundEntry::Groups { name, .. } => format!("<{name}>"),
            })
            .collect()
    }

    const ORU: &str = "MSH|^~\\&|LAB|FAC|EHR|FAC|20240102030405||ORU^R01^ORU_R01|MSG1|P|2.5.1\r\
PID|1||12345^^^HOSP^MR||SMITH^JOHN\r\
OBR|1|ORD1||24331-1^Lipid Panel^LN\r\
OBX|1|NM|2093-3^Cholesterol^LN||187|mg/dL|<200|N|||F\r\
OBX|2|NM|2571-8^Triglyceride^LN||150|mg/dL|<150|N|||F\r";

    #[test]
    fn oru_binds_nested_groups() {
        let defs = defs();
        let msg = parse(ORU).unwrap();
        let (bound, issues) = bind_text(&defs, &msg);

        assert_eq!(bound.event.as_deref(), Some("ORU_R01"));
        assert_eq!(entry_ids(&bound.root), ["MSH", "<PATIENT_RESULT>"]);

        let BoundEntry::Groups { instances, .. } = &bound.root.entries[1] else {
            panic!("expected group")
        };
        let pr = &instances[0];
        assert_eq!(entry_ids(pr), ["<PATIENT>", "<ORDER_OBSERVATION>"]);

        let BoundEntry::Groups { instances: oo, .. } = &pr.entries[1] else {
            panic!()
        };
        // ORDER_OBSERVATION: OBR + OBSERVATION group ×2 (one per OBX).
        let ids = entry_ids(&oo[0]);
        assert!(ids.contains(&"OBR".to_string()), "{ids:?}");
        assert!(ids.contains(&"<OBSERVATION>".to_string()), "{ids:?}");
        assert!(bound.unexpected.is_empty());
        // ORC is required in ORDER_OBSERVATION per 2.5.1? It's usage O there; no errors expected.
        assert!(
            issues.iter().all(|i| i.severity != Severity::Error),
            "{issues:?}"
        );
    }

    #[test]
    fn z_segments_are_preserved_with_info() {
        let defs = defs();
        let text = ORU.replace("OBR|", "ZBX|custom|data\rOBR|");
        let msg = parse(&text).unwrap();
        let (bound, issues) = bind_text(&defs, &msg);
        assert_eq!(bound.unexpected.len(), 1);
        assert_eq!(bound.unexpected[0].segment.id, "ZBX");
        assert!(issues
            .iter()
            .any(|i| i.severity == Severity::Info && i.message.contains("ZBX")));
    }

    #[test]
    fn missing_required_segment_reported() {
        let defs = defs();
        // ADT_A01 requires EVN and PV1; omit both.
        let text = "MSH|^~\\&|A|F|B|F|20240101||ADT^A01^ADT_A01|1|P|2.5.1\rPID|1||1^^^H^MR\r";
        let msg = parse(text).unwrap();
        let (_, issues) = bind_text(&defs, &msg);
        let errors: Vec<&Issue> = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .collect();
        assert!(
            errors.iter().any(|i| i.message.contains("EVN")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|i| i.message.contains("PV1")),
            "{errors:?}"
        );
    }

    #[test]
    fn out_of_place_known_segment_becomes_unexpected() {
        let defs = defs();
        // A second PID trailing an ADT_A01 (PID does not repeat there).
        let text = "MSH|^~\\&|A|F|B|F|20240101||ADT^A01^ADT_A01|1|P|2.5.1\rEVN|A01|20240101\rPID|1||1^^^H^MR\rPV1|1|I\rPID|2||2^^^H^MR\r";
        let msg = parse(text).unwrap();
        let (bound, issues) = bind_text(&defs, &msg);
        assert_eq!(bound.unexpected.len(), 1);
        assert_eq!(bound.unexpected[0].segment.id, "PID");
        assert!(issues
            .iter()
            .any(|i| i.severity == Severity::Warning && i.message.contains("not expected")));
    }

    #[test]
    fn event_fallback_without_struct_id() {
        let defs = defs();
        // MSH-9.3 empty -> fall back to TYPE_TRIGGER.
        let text =
            "MSH|^~\\&|A|F|B|F|20240101||ADT^A01|1|P|2.5.1\rEVN|A01|20240101\rPID|1\rPV1|1\r";
        let msg = parse(text).unwrap();
        let (sel, _) = select_event(&defs, &msg.segments[0]);
        assert_eq!(sel.unwrap().0, "ADT_A01");
    }

    #[test]
    fn unknown_event_is_error_not_panic() {
        let defs = defs();
        let text = "MSH|^~\\&|A|F|B|F|20240101||QQQ^Z99|1|P|2.5.1\r";
        let msg = parse(text).unwrap();
        let (sel, issues) = select_event(&defs, &msg.segments[0]);
        assert!(sel.is_none());
        assert_eq!(issues[0].severity, Severity::Error);
    }
}
