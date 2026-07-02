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

//! JSON tab: flattened collapsible tree with type-colored leaves,
//! expand/collapse all, copy JSON.

use crate::convert::Converted;
use crate::js;
use crate::jsontree;
use crate::state::AppState;
use dioxus::prelude::*;
use std::rc::Rc;

#[component]
pub fn JsonTab(conv: ReadSignal<Rc<Converted>>) -> Element {
    let mut state = AppState::use_ctx();
    let conv_rc = conv.read().clone();
    let collapsed = state
        .collapsed
        .read()
        .clone()
        .unwrap_or_else(|| jsontree::default_collapsed(&conv_rc.nodes));

    let sel_node_id = state
        .sel_seg
        .read()
        .as_ref()
        .and_then(|code| conv_rc.seg_occurrences.get(&(code.clone(), 1)))
        .cloned();

    // Depth-1 segment nodes flagged by an issue get the "! spec issue" note.
    let flagged: Vec<&str> = conv_rc
        .segments
        .iter()
        .filter(|s| s.flagged)
        .map(|s| s.code.as_str())
        .collect();

    let copy_label = if *state.copied.read() {
        "Copied ✓"
    } else {
        "Copy JSON"
    };

    let conv_for_copy = conv_rc.clone();
    let copy_json = move |_| {
        let pretty = serde_json::to_string_pretty(&conv_for_copy.message).unwrap_or_default();
        js::copy_text(&pretty);
        state.copied.set(true);
        spawn(async move {
            js::sleep_ms(1400).await;
            state.copied.set(false);
        });
    };

    let conv_for_default = conv_rc.clone();
    let collapse_all = move |_| {
        state
            .collapsed
            .set(Some(jsontree::default_collapsed(&conv_for_default.nodes)));
    };

    rsx! {
        div { class: "pane-pad",
            div { class: "json-toolbar",
                button {
                    class: "btn-sm",
                    onclick: move |_| state.collapsed.set(Some(Default::default())),
                    "Expand all"
                }
                button { class: "btn-sm", onclick: collapse_all, "Collapse all" }
                div { class: "spacer" }
                button { class: "btn-sm primary", onclick: copy_json, "{copy_label}" }
            }
            div { class: "json-tree",
                for node in conv_rc.nodes.iter() {
                    if !node.ancestors.iter().any(|a| collapsed.contains(a)) {
                        {
                            let is_open = node.collapsible && !collapsed.contains(&node.id);
                            let selected = sel_node_id.as_deref() == Some(node.id.as_str());
                            let seg_flag = node.depth == 1 && flagged.contains(&node.key.as_str());
                            let chevron = if node.collapsible {
                                if is_open { "▾" } else { "▸" }
                            } else {
                                ""
                            };
                            let node_id = node.id.clone();
                            let collapsible = node.collapsible;
                            let toggle = move |_| {
                                if !collapsible {
                                    return;
                                }
                                let conv = conv.read().clone();
                                let mut set = state.collapsed.read().clone().unwrap_or_else(|| {
                                    jsontree::default_collapsed(&conv.nodes)
                                });
                                if !set.remove(&node_id) {
                                    set.insert(node_id.clone());
                                }
                                state.collapsed.set(Some(set));
                            };
                            rsx! {
                                div {
                                    id: "json-{node.id}",
                                    class: if selected { "json-row selected" } else { "json-row" },
                                    style: "padding-left: {node.depth * 16 + 6}px;",
                                    span {
                                        class: if node.collapsible { "chev open-toggle" } else { "chev" },
                                        onclick: toggle,
                                        "{chevron}"
                                    }
                                    span { class: "json-key", "{node.key}" }
                                    if !node.collapsible {
                                        span { class: "json-sep", ":" }
                                    }
                                    if node.collapsible {
                                        span { class: "json-val preview", "{node.preview}" }
                                    } else {
                                        span {
                                            class: "json-val {node.leaf_class}",
                                            "{node.leaf.as_deref().unwrap_or_default()}"
                                        }
                                    }
                                    if seg_flag {
                                        span { class: "row-flag", "! spec issue" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
