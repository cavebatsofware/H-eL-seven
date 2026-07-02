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

//! Left navigator rail: message meta, segment list, conversion issues,
//! unmapped segments.

use crate::components::badges::SeverityBadge;
use crate::convert::Converted;
use crate::state::AppState;
use dioxus::prelude::*;
use std::rc::Rc;

#[component]
pub fn Rail(conv: ReadSignal<Rc<Converted>>) -> Element {
    let state = AppState::use_ctx();
    let conv = conv.read().clone();
    let sel = state.sel_seg.read().clone();

    let meta = [
        ("Event", conv.meta.event.clone()),
        ("Control ID", conv.meta.control_id.clone()),
        ("HL7", format!("v{}", conv.meta.hl7_version)),
        ("From", conv.meta.from.clone()),
        ("To", conv.meta.to.clone()),
        ("Received", conv.meta.received.clone()),
    ];

    rsx! {
        aside { class: "rail", id: "rail-scroll",
            div { class: "rail-section",
                div { class: "rail-eyebrow", style: "margin-bottom: 8px;", "Message" }
                for (k, v) in meta {
                    div { class: "meta-row",
                        span { class: "meta-k", "{k}" }
                        span { class: "meta-v", "{v}" }
                    }
                }
            }

            div { class: "rail-divider" }

            div { class: "rail-head",
                div { class: "rail-eyebrow", "Segments" }
                span { class: "rail-count", "{conv.seg_count} segments" }
            }
            for seg in conv.segments.iter() {
                {
                    let code = seg.code.clone();
                    let selected = sel.as_deref() == Some(seg.code.as_str());
                    rsx! {
                        button {
                            class: if selected { "seg-btn selected" } else { "seg-btn" },
                            onclick: move |_| state.select_seg(&code),
                            span { class: "seg-code", "{seg.code}" }
                            span { class: "seg-label", "{seg.label}" }
                            if seg.flagged {
                                span { class: "flag-pill", "!" }
                            }
                            span { class: "seg-count", "{seg.count}" }
                        }
                    }
                }
            }

            div { class: "rail-divider later" }

            div { class: "rail-head",
                div { class: "rail-eyebrow", "Conversion issues" }
                span { class: "rail-count", "{conv.issues.len()} total" }
            }
            for issue in conv.issues.iter() {
                {
                    let seg = issue.seg.clone();
                    let occ = issue.occ;
                    rsx! {
                        button {
                            class: "issue-btn",
                            title: "{issue.message}",
                            onclick: move |_| state.jump_to(&seg, occ),
                            div { class: "issue-top",
                                SeverityBadge { severity: issue.severity.clone() }
                                span { class: "issue-loc", "{issue.location}" }
                            }
                            div { class: "issue-msg", "{issue.message}" }
                        }
                    }
                }
            }

            if !conv.unexpected.is_empty() {
                div { class: "rail-section", style: "padding: 14px 16px 6px;",
                    div { class: "rail-eyebrow danger", "Unmapped" }
                }
                for un in conv.unexpected.iter() {
                    {
                        let seg = un.segment.clone();
                        rsx! {
                            button {
                                class: "unmapped-btn",
                                onclick: move |_| state.jump_to(&seg, 1),
                                div { class: "unmapped-code",
                                    "{un.segment} "
                                    span { class: "pos", "@ pos {un.position}" }
                                }
                                div { class: "unmapped-detail", "{un.detail}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
