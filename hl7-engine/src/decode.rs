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

//! Typed decoding of segments: raw positional values -> named, typed JSON,
//! driven by the segment/data-type definitions, with validation issues
//! (usage, cardinality, length, table membership) recorded along the way.
//!
//! Validation policy (lenient always):
//! - missing required field            -> error
//! - repeats on a non-repeating field  -> warning (data kept as array)
//! - value not in an HL7-typed table   -> warning (User tables are site-defined: skipped)
//! - malformed NM/SI/date-time         -> warning (raw string kept)
//! - length over spec                  -> info (spec lengths are widely ignored)

use crate::defs::{DataTypeDef, Definitions, FieldDef, SegmentDef};
use crate::escape;
use crate::issue::{Issue, Severity};
use crate::syntax::{Delimiters, RawComponent, RawRepeat, RawSegment};
use serde_json::{json, Map, Value};

pub struct Decoder<'d> {
    pub defs: &'d Definitions,
    pub delims: Delimiters,
}

impl<'d> Decoder<'d> {
    /// Decode one segment instance into a JSON object keyed by field names.
    /// `instance` is the 1-based instance number, used only in issue locations.
    pub fn segment(
        &self,
        seg: &RawSegment<'_>,
        def: &SegmentDef,
        instance: usize,
        issues: &mut Vec<Issue>,
    ) -> Value {
        let mut out = Map::new();
        let keys = field_keys(def);
        let field_count = def.fields.len().max(seg.fields.len());

        for idx in 1..=field_count {
            let loc = format!("{}[{}]-{}", seg.id, instance, idx);
            let raw = seg.fields.get(idx - 1);
            let fdef = def.fields.get(idx - 1);

            let Some(fdef) = fdef else {
                // More fields than the spec defines.
                if raw.is_some_and(|f| !field_is_empty(f)) {
                    issues.push(Issue::new(
                        Severity::Warning,
                        loc.clone(),
                        format!(
                            "field {idx} is beyond the {} definition of {}",
                            self.defs.version, seg.id
                        ),
                    ));
                    if let Some(raw) = raw {
                        out.insert(format!("field{idx}"), self.field_positional(raw));
                    }
                }
                continue;
            };

            let empty = raw.is_none_or(field_is_empty);
            if empty {
                if fdef.usage == "R" && !(seg.id == "MSH" && idx <= 2) {
                    issues.push(Issue::new(
                        Severity::Error,
                        loc,
                        format!("required field {} ({}) is empty", idx, fdef.name),
                    ));
                }
                continue;
            }
            let raw = raw.unwrap();

            // MSH-1/MSH-2 are the delimiters themselves: raw, never unescaped.
            if seg.id == "MSH" && idx <= 2 {
                out.insert(
                    keys[idx - 1].clone(),
                    Value::String(raw.first().to_string()),
                );
                continue;
            }

            // OBX-5 is typed at runtime by OBX-2.
            let data_type = if seg.id == "OBX" && idx == 5 {
                let declared = seg.field(2).map(|f| f.first()).unwrap_or("");
                if self.defs.data_types.contains_key(declared) {
                    declared.to_string()
                } else {
                    fdef.data_type.clone()
                }
            } else {
                fdef.data_type.clone()
            };

            let mut reps: Vec<Value> = Vec::new();
            for (r, rep) in raw.repeats.iter().enumerate() {
                let rep_loc = if raw.repeats.len() > 1 {
                    format!("{loc}[{}]", r + 1)
                } else {
                    loc.clone()
                };
                if let Some(v) = self.repeat(rep, &data_type, fdef, &rep_loc, issues) {
                    reps.push(v);
                }
            }
            if reps.is_empty() {
                continue;
            }

            let repeats_allowed = fdef.rpt != "1";
            let value = if repeats_allowed {
                Value::Array(reps)
            } else if reps.len() == 1 {
                reps.into_iter().next().unwrap()
            } else {
                issues.push(Issue::new(
                    Severity::Warning,
                    loc,
                    format!(
                        "field {} ({}) does not repeat, but has {} repetitions",
                        idx,
                        fdef.name,
                        reps.len()
                    ),
                ));
                Value::Array(reps)
            };
            out.insert(keys[idx - 1].clone(), value);
        }
        Value::Object(out)
    }

    /// One repetition of a field, decoded per its data type.
    fn repeat(
        &self,
        rep: &RawRepeat<'_>,
        data_type: &str,
        fdef: &FieldDef,
        loc: &str,
        issues: &mut Vec<Issue>,
    ) -> Option<Value> {
        // An explicit null ("") filling the whole repetition nulls the field,
        // even when its data type is composite.
        if rep.components.len() == 1
            && rep.components[0].subcomponents.len() == 1
            && rep.components[0].subcomponents[0] == "\"\""
        {
            return Some(Value::Null);
        }
        match self.defs.data_types.get(data_type) {
            Some(dt) if !dt.components.is_empty() => self.composite(rep, dt, loc, issues),
            Some(_) => {
                // Primitive data type.
                if rep.components.len() > 1 || rep.components[0].subcomponents.len() > 1 {
                    issues.push(Issue::new(
                        Severity::Warning,
                        loc.to_string(),
                        format!("components present in primitive ({data_type}) field"),
                    ));
                    return Some(self.repeat_positional(rep));
                }
                self.leaf(
                    rep.components[0].subcomponents[0],
                    data_type,
                    fdef,
                    loc,
                    issues,
                )
            }
            None => {
                // VARIES or an unknown type: keep everything, positionally.
                if rep.components.len() == 1 && rep.components[0].subcomponents.len() == 1 {
                    self.leaf(rep.components[0].subcomponents[0], "ST", fdef, loc, issues)
                } else {
                    Some(self.repeat_positional(rep))
                }
            }
        }
    }

    fn composite(
        &self,
        rep: &RawRepeat<'_>,
        dt: &DataTypeDef,
        loc: &str,
        issues: &mut Vec<Issue>,
    ) -> Option<Value> {
        let mut out = Map::new();
        let keys = component_keys(dt);
        for (j, comp) in rep.components.iter().enumerate() {
            let comp_loc = format!("{loc}.{}", j + 1);
            let Some(cdef) = dt.components.get(j) else {
                if !component_is_empty(comp) {
                    issues.push(Issue::new(
                        Severity::Warning,
                        comp_loc,
                        format!(
                            "component {} is beyond the definition of {}",
                            j + 1,
                            dt.name
                        ),
                    ));
                    out.insert(
                        format!("component{}", j + 1),
                        self.component_positional(comp),
                    );
                }
                continue;
            };
            if component_is_empty(comp) {
                // Component-level "R" usage is not enforced: the spec marks
                // conditional/required components inconsistently across types.
                continue;
            }
            if comp.subcomponents.len() == 1 && comp.subcomponents[0] == "\"\"" {
                out.insert(keys[j].clone(), Value::Null);
                continue;
            }
            let value = match self.defs.data_types.get(&cdef.data_type) {
                Some(sub) if !sub.components.is_empty() => {
                    self.subcomposite(comp, sub, &comp_loc, issues)
                }
                _ => {
                    if comp.subcomponents.len() > 1 {
                        issues.push(Issue::new(
                            Severity::Warning,
                            comp_loc.clone(),
                            format!(
                                "subcomponents present in primitive ({}) component",
                                cdef.data_type
                            ),
                        ));
                        Some(self.component_positional(comp))
                    } else {
                        self.leaf(
                            comp.subcomponents[0],
                            &cdef.data_type,
                            cdef,
                            &comp_loc,
                            issues,
                        )
                    }
                }
            };
            if let Some(v) = value {
                out.insert(keys[j].clone(), v);
            }
        }
        (!out.is_empty()).then_some(Value::Object(out))
    }

    /// Third level: subcomponents against the component's own data type.
    fn subcomposite(
        &self,
        comp: &RawComponent<'_>,
        dt: &DataTypeDef,
        loc: &str,
        issues: &mut Vec<Issue>,
    ) -> Option<Value> {
        let mut out = Map::new();
        let keys = component_keys(dt);
        for (k, sub) in comp.subcomponents.iter().enumerate() {
            let sub_loc = format!("{loc}.{}", k + 1);
            let Some(sdef) = dt.components.get(k) else {
                if !sub.is_empty() {
                    issues.push(Issue::new(
                        Severity::Warning,
                        sub_loc,
                        format!(
                            "subcomponent {} is beyond the definition of {}",
                            k + 1,
                            dt.name
                        ),
                    ));
                    out.insert(
                        format!("subcomponent{}", k + 1),
                        Value::String(escape::decode(sub, &self.delims).into_owned()),
                    );
                }
                continue;
            };
            if let Some(v) = self.leaf(sub, &sdef.data_type, sdef, &sub_loc, issues) {
                out.insert(keys[k].clone(), v);
            }
        }
        (!out.is_empty()).then_some(Value::Object(out))
    }

    /// A leaf value: escape-decode, then type and validate it.
    fn leaf(
        &self,
        raw: &str,
        data_type: &str,
        def: &FieldDef,
        loc: &str,
        issues: &mut Vec<Issue>,
    ) -> Option<Value> {
        if raw.is_empty() {
            return None;
        }
        // HL7 explicit null: a pair of double quotes.
        if raw == "\"\"" {
            return Some(Value::Null);
        }
        let text = escape::decode(raw, &self.delims);

        if def.length > 0 && text.chars().count() as i64 > def.length {
            issues.push(Issue::new(
                Severity::Info,
                loc.to_string(),
                format!(
                    "value exceeds spec length {} ({} chars)",
                    def.length,
                    text.chars().count()
                ),
            ));
        }
        if let Some(table_id) = &def.table {
            if let Some(table) = self.defs.tables.get(table_id) {
                if table.table_type == "HL7"
                    && !table.values.is_empty()
                    && !table.values.contains_key(text.as_ref())
                {
                    issues.push(Issue::new(
                        Severity::Warning,
                        loc.to_string(),
                        format!(
                            "value {:?} not in HL7 table {} ({})",
                            text.as_ref(),
                            table_id,
                            table.name
                        ),
                    ));
                }
            }
        }

        let value = match data_type {
            "NM" => match text.parse::<f64>() {
                Ok(n) if n.is_finite() => serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .unwrap_or_else(|| Value::String(text.to_string())),
                _ => {
                    issues.push(Issue::new(
                        Severity::Warning,
                        loc.to_string(),
                        format!("NM value {:?} is not numeric", text.as_ref()),
                    ));
                    Value::String(text.into_owned())
                }
            },
            "SI" => match text.trim().parse::<u64>() {
                Ok(n) => Value::Number(n.into()),
                Err(_) => {
                    issues.push(Issue::new(
                        Severity::Warning,
                        loc.to_string(),
                        format!("SI value {:?} is not a sequence number", text.as_ref()),
                    ));
                    Value::String(text.into_owned())
                }
            },
            "DTM" | "TS" => datetime_to_iso(&text)
                .map(Value::String)
                .unwrap_or_else(|| {
                    issues.push(Issue::new(
                        Severity::Warning,
                        loc.to_string(),
                        format!("invalid date/time {:?}", text.as_ref()),
                    ));
                    Value::String(text.into_owned())
                }),
            "DT" => date_to_iso(&text).map(Value::String).unwrap_or_else(|| {
                issues.push(Issue::new(
                    Severity::Warning,
                    loc.to_string(),
                    format!("invalid date {:?}", text.as_ref()),
                ));
                Value::String(text.into_owned())
            }),
            "TM" => time_to_iso(&text).map(Value::String).unwrap_or_else(|| {
                issues.push(Issue::new(
                    Severity::Warning,
                    loc.to_string(),
                    format!("invalid time {:?}", text.as_ref()),
                ));
                Value::String(text.into_owned())
            }),
            _ => Value::String(text.into_owned()),
        };
        Some(value)
    }

    /// Positional (unnamed) rendering, for data with no usable definition:
    /// nested arrays collapsed to a string when only one leaf exists.
    pub fn field_positional(&self, field: &crate::syntax::RawField<'_>) -> Value {
        let reps: Vec<Value> = field
            .repeats
            .iter()
            .map(|r| self.repeat_positional(r))
            .collect();
        collapse(reps)
    }

    fn repeat_positional(&self, rep: &RawRepeat<'_>) -> Value {
        let comps: Vec<Value> = rep
            .components
            .iter()
            .map(|c| self.component_positional(c))
            .collect();
        collapse(comps)
    }

    fn component_positional(&self, comp: &RawComponent<'_>) -> Value {
        let subs: Vec<Value> = comp
            .subcomponents
            .iter()
            .map(|s| Value::String(escape::decode(s, &self.delims).into_owned()))
            .collect();
        collapse(subs)
    }

    /// Whole segment, positionally - used for unexpected segments and Z-segments.
    pub fn segment_positional(&self, seg: &RawSegment<'_>, position: usize) -> Value {
        let fields: Vec<Value> = seg
            .fields
            .iter()
            .map(|f| {
                if seg.id == "MSH" {
                    Value::String(f.first().to_string())
                } else {
                    self.field_positional(f)
                }
            })
            .collect();
        json!({ "segment": seg.id, "position": position, "fields": fields })
    }
}

fn collapse(mut items: Vec<Value>) -> Value {
    if items.len() == 1 {
        items.pop().unwrap()
    } else {
        Value::Array(items)
    }
}

fn field_is_empty(field: &crate::syntax::RawField<'_>) -> bool {
    field
        .repeats
        .iter()
        .all(|r| r.components.iter().all(component_is_empty))
}

fn component_is_empty(comp: &RawComponent<'_>) -> bool {
    comp.subcomponents.iter().all(|s| s.is_empty())
}

/// "Patient Identifier List" -> "patientIdentifierList"; collisions get the
/// 1-based position appended; empty names become "field{n}".
pub fn field_keys(def: &SegmentDef) -> Vec<String> {
    dedupe_keys(def.fields.iter().map(|f| f.name.as_str()), "field")
}

pub fn component_keys(dt: &DataTypeDef) -> Vec<String> {
    dedupe_keys(dt.components.iter().map(|c| c.name.as_str()), "component")
}

fn dedupe_keys<'a>(names: impl Iterator<Item = &'a str>, fallback: &str) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (i, name) in names.enumerate() {
        let mut key = to_camel(name);
        if key.is_empty() {
            key = format!("{fallback}{}", i + 1);
        }
        if !seen.insert(key.clone()) {
            key = format!("{key}{}", i + 1);
            seen.insert(key.clone());
        }
        keys.push(key);
    }
    keys
}

pub fn to_camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut new_word = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if out.is_empty() {
                out.extend(ch.to_lowercase());
            } else if new_word {
                out.extend(ch.to_uppercase());
            } else {
                out.extend(ch.to_lowercase());
            }
            new_word = false;
        } else {
            new_word = !out.is_empty();
        }
    }
    out
}

/// DTM: YYYY[MM[DD[HH[MM[SS[.f{1,4}]]]]]][±ZZZZ] -> progressive ISO 8601.
pub fn datetime_to_iso(s: &str) -> Option<String> {
    let (body, offset) = split_offset(s);
    let (main, frac) = match body.split_once('.') {
        Some((m, f)) => (m, Some(f)),
        None => (body, None),
    };
    if main.is_empty() || !main.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if let Some(f) = frac {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }

    let mut out = String::new();
    let pieces: [(usize, usize, u32, u32, char); 6] = [
        (0, 4, 1, 9999, '\0'), // year
        (4, 6, 1, 12, '-'),    // month
        (6, 8, 1, 31, '-'),    // day
        (8, 10, 0, 23, 'T'),   // hour
        (10, 12, 0, 59, ':'),  // minute
        (12, 14, 0, 60, ':'),  // second (60 = leap)
    ];
    if main.len() > 14 || main.len() < 4 {
        return None;
    }
    for &(start, end, min, max, sep) in &pieces {
        if main.len() < end {
            // Prefix precision is fine, but it must break on a piece boundary.
            if main.len() > start {
                return None;
            }
            break;
        }
        let n: u32 = main[start..end].parse().ok()?;
        if n < min || n > max {
            return None;
        }
        if sep != '\0' {
            out.push(sep);
        }
        out.push_str(&main[start..end]);
    }
    if let Some(f) = frac {
        if main.len() != 14 {
            return None;
        }
        out.push('.');
        out.push_str(f);
    }
    if let Some(off) = offset {
        if main.len() < 10 {
            return None;
        }
        out.push_str(&off);
    }
    Some(out)
}

pub fn date_to_iso(s: &str) -> Option<String> {
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match s.len() {
        4 | 6 | 8 => datetime_to_iso(s),
        _ => None,
    }
}

/// TM: HH[MM[SS[.f]]][±ZZZZ] -> "HH:MM:SS[.f][±HH:MM]".
pub fn time_to_iso(s: &str) -> Option<String> {
    let (body, offset) = split_offset(s);
    let (main, frac) = match body.split_once('.') {
        Some((m, f)) => (m, Some(f)),
        None => (body, None),
    };
    if !main.bytes().all(|b| b.is_ascii_digit()) || !matches!(main.len(), 2 | 4 | 6) {
        return None;
    }
    let mut out = String::new();
    for (i, max) in [(0usize, 23u32), (2, 59), (4, 60)] {
        if main.len() < i + 2 {
            break;
        }
        let n: u32 = main[i..i + 2].parse().ok()?;
        if n > max {
            return None;
        }
        if i > 0 {
            out.push(':');
        }
        out.push_str(&main[i..i + 2]);
    }
    if let Some(f) = frac {
        if main.len() != 6 || f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out.push('.');
        out.push_str(f);
    }
    if let Some(off) = offset {
        out.push_str(&off);
    }
    Some(out)
}

/// Split a trailing ±ZZZZ timezone; returns (body, Some("±HH:MM")).
fn split_offset(s: &str) -> (&str, Option<String>) {
    if let Some(idx) = s.rfind(['+', '-']) {
        let off = &s[idx + 1..];
        if off.len() == 4 && off.bytes().all(|b| b.is_ascii_digit()) {
            let (h, m) = off.split_at(2);
            let sign = &s[idx..idx + 1];
            if h.parse::<u32>().map(|h| h <= 14).unwrap_or(false)
                && m.parse::<u32>().map(|m| m <= 59).unwrap_or(false)
            {
                return (&s[..idx], Some(format!("{sign}{h}:{m}")));
            }
        }
    }
    (s, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_names() {
        assert_eq!(to_camel("Patient Identifier List"), "patientIdentifierList");
        assert_eq!(to_camel("Set ID - PID"), "setIdPid");
        assert_eq!(to_camel("Date/Time of Birth"), "dateTimeOfBirth");
        assert_eq!(to_camel("Family Name"), "familyName");
        assert_eq!(to_camel(""), "");
    }

    #[test]
    fn datetimes() {
        assert_eq!(
            datetime_to_iso("20240102030405").as_deref(),
            Some("2024-01-02T03:04:05")
        );
        assert_eq!(
            datetime_to_iso("202401020304").as_deref(),
            Some("2024-01-02T03:04")
        );
        assert_eq!(datetime_to_iso("20240102").as_deref(), Some("2024-01-02"));
        assert_eq!(datetime_to_iso("202401").as_deref(), Some("2024-01"));
        assert_eq!(datetime_to_iso("2024").as_deref(), Some("2024"));
        assert_eq!(
            datetime_to_iso("20240102030405.123-0500").as_deref(),
            Some("2024-01-02T03:04:05.123-05:00")
        );
        assert_eq!(
            datetime_to_iso("2024010203-0500").as_deref(),
            Some("2024-01-02T03-05:00")
        );
        assert_eq!(datetime_to_iso("20241302030405"), None); // month 13
        assert_eq!(datetime_to_iso("2024010"), None); // broken boundary
        assert_eq!(datetime_to_iso("banana"), None);
        assert_eq!(datetime_to_iso(""), None);
    }

    #[test]
    fn times_and_dates() {
        assert_eq!(time_to_iso("0304").as_deref(), Some("03:04"));
        assert_eq!(
            time_to_iso("030405.5+0100").as_deref(),
            Some("03:04:05.5+01:00")
        );
        assert_eq!(time_to_iso("25"), None);
        assert_eq!(date_to_iso("19800101").as_deref(), Some("1980-01-01"));
        assert_eq!(date_to_iso("198001").as_deref(), Some("1980-01"));
        assert_eq!(date_to_iso("1980010"), None);
    }
}
