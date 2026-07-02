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

//! Narrative tab: generated section cards.

use crate::convert::Converted;
use crate::narrative;
use crate::state::AppState;
use dioxus::prelude::*;
use std::rc::Rc;

#[component]
pub fn NarrativeTab(conv: ReadSignal<Rc<Converted>>) -> Element {
    let state = AppState::use_ctx();
    let conv = conv.read().clone();
    let defs = state
        .defs_cache
        .read()
        .get(&conv.meta.defs_version)
        .cloned();
    let sections = narrative::build(&conv, defs.as_deref());

    rsx! {
        div { class: "narr-wrap",
            for section in sections {
                div { class: "narr-card",
                    div { class: "narr-head",
                        div { class: "narr-eyebrow", "Generated narrative" }
                        div { class: "narr-title", "{section.title}" }
                        div { class: "narr-sub", "{section.sub}" }
                    }
                    div { class: "narr-body",
                        for (k, v) in section.rows {
                            div { class: "narr-row",
                                span { class: "narr-k", "{k}" }
                                span { class: "narr-v", "{v}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
