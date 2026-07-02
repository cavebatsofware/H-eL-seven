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

//! HL7 tab: the raw message, line by line, with issue/unmapped markers.

use crate::convert::{Converted, LineMark};
use crate::state::AppState;
use dioxus::prelude::*;
use std::rc::Rc;

#[component]
pub fn Hl7Tab(conv: ReadSignal<Rc<Converted>>) -> Element {
    let state = AppState::use_ctx();
    let conv = conv.read().clone();
    let sel = state.sel_seg.read().clone();

    rsx! {
        div { class: "pane-pad",
            div { class: "caption",
                "Raw message - {conv.seg_count} segments across {conv.raw_lines.len()} lines. \
                 Flagged lines carry a spec issue or are unmapped; click to inspect."
            }
            div { class: "hl7-block",
                for line in conv.raw_lines.iter() {
                    {
                        let code = line.code.clone();
                        let selected = sel.as_deref() == Some(line.code.as_str());
                        let row_class = match (selected, line.mark) {
                            (true, _) => "hl7-line selected",
                            (false, LineMark::Unmapped) => "hl7-line unmapped",
                            _ => "hl7-line",
                        };
                        let dot_class = match line.mark {
                            LineMark::Unmapped => "line-dot -unmapped",
                            LineMark::Issue => "line-dot -issue",
                            LineMark::None => "line-dot",
                        };
                        rsx! {
                            div {
                                id: "hl7-{line.code}-{line.occ}",
                                class: "{row_class}",
                                title: "{line.title}",
                                onclick: move |_| state.select_seg(&code),
                                span { class: "line-no", "{line.n}" }
                                span { class: "{dot_class}" }
                                span { class: "line-code", "{line.code}" }
                                span { class: "line-rest", "{line.rest}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
