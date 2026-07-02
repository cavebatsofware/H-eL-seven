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

//! Flatten a serde_json::Value into displayable rows (the prototype's
//! buildNodes): expand/collapse and scroll-to-node stay simple row logic
//! instead of recursive components.

use serde_json::Value;
use std::collections::HashSet;

pub struct JsonNode {
    /// "root", "root.PID", "root.PID.patientName.0", …
    pub id: String,
    pub key: String,
    pub depth: usize,
    pub collapsible: bool,
    /// "{ 12 }" or "[ 3 ]" for containers.
    pub preview: String,
    /// Formatted leaf value (strings quoted), None for containers.
    pub leaf: Option<String>,
    /// CSS class for leaf type coloring.
    pub leaf_class: &'static str,
    pub ancestors: Vec<String>,
}

pub fn flatten(root_key: &str, value: &Value) -> Vec<JsonNode> {
    let mut nodes = Vec::new();
    walk(root_key, value, 0, "root", &[], &mut nodes);
    nodes
}

fn walk(
    key: &str,
    value: &Value,
    depth: usize,
    id: &str,
    ancestors: &[String],
    out: &mut Vec<JsonNode>,
) {
    let (collapsible, preview) = match value {
        Value::Array(a) => (true, format!("[ {} ]", a.len())),
        Value::Object(o) => (true, format!("{{ {} }}", o.len())),
        _ => (false, String::new()),
    };
    let (leaf, leaf_class) = if collapsible {
        (None, "")
    } else {
        match value {
            Value::String(s) => (Some(format!("\"{s}\"")), "leaf-string"),
            Value::Number(n) => (Some(n.to_string()), "leaf-number"),
            Value::Bool(b) => (Some(b.to_string()), "leaf-bool"),
            _ => (Some("null".to_string()), "leaf-null"),
        }
    };
    out.push(JsonNode {
        id: id.to_string(),
        key: key.to_string(),
        depth,
        collapsible,
        preview,
        leaf,
        leaf_class,
        ancestors: ancestors.to_vec(),
    });
    if collapsible {
        let mut next_ancestors = ancestors.to_vec();
        next_ancestors.push(id.to_string());
        match value {
            Value::Array(a) => {
                for (i, v) in a.iter().enumerate() {
                    let child_id = format!("{id}.{i}");
                    walk(
                        &i.to_string(),
                        v,
                        depth + 1,
                        &child_id,
                        &next_ancestors,
                        out,
                    );
                }
            }
            Value::Object(o) => {
                for (k, v) in o {
                    let child_id = format!("{id}.{k}");
                    walk(k, v, depth + 1, &child_id, &next_ancestors, out);
                }
            }
            _ => unreachable!(),
        }
    }
}

/// The design's default collapse state: every collapsible node at depth ≥ 1.
pub fn default_collapsed(nodes: &[JsonNode]) -> HashSet<String> {
    nodes
        .iter()
        .filter(|n| n.collapsible && n.depth >= 1)
        .map(|n| n.id.clone())
        .collect()
}
