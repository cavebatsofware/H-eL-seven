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

//! Explorer screen: 272px navigator rail + tabbed main pane.
//!
//! Only mounted by App when a conversion exists (view can only reach
//! Explorer through a successful convert), so the unwrap in the memo is
//! safe by construction.

use crate::components::rail::Rail;
use crate::components::tab_hl7::Hl7Tab;
use crate::components::tab_json::JsonTab;
use crate::components::tab_narrative::NarrativeTab;
use crate::components::tabs::{TabContent, TabList, TabTrigger, Tabs};
use crate::convert::Converted;
use crate::state::{AppState, Tab};
use dioxus::prelude::*;
use std::rc::Rc;

#[component]
pub fn Explorer() -> Element {
    let mut state = AppState::use_ctx();
    let conv: Memo<Rc<Converted>> = use_memo(move || {
        state
            .converted
            .read()
            .clone()
            .expect("explorer needs a conversion")
    });

    let tab_value = use_memo(move || {
        Some(
            match *state.tab.read() {
                Tab::Hl7 => "hl7",
                Tab::Json => "json",
                Tab::Narrative => "narrative",
            }
            .to_string(),
        )
    });

    rsx! {
        div { class: "explorer-grid",
            Rail { conv }
            main { class: "main-pane",
                Tabs {
                    value: tab_value,
                    on_value_change: move |v: String| {
                        state.tab.set(match v.as_str() {
                            "json" => Tab::Json,
                            "narrative" => Tab::Narrative,
                            _ => Tab::Hl7,
                        });
                    },
                    TabList {
                        TabTrigger { value: "hl7", index: 0usize, "HL7" }
                        TabTrigger { value: "json", index: 1usize, "JSON" }
                        TabTrigger { value: "narrative", index: 2usize, "Narrative" }
                    }
                    TabContent { value: "hl7", index: 0usize,
                        div { class: "tab-scroll", id: "hl7-scroll",
                            Hl7Tab { conv }
                        }
                    }
                    TabContent { value: "json", index: 1usize,
                        div { class: "tab-scroll", id: "json-scroll",
                            JsonTab { conv }
                        }
                    }
                    TabContent { value: "narrative", index: 2usize,
                        div { class: "tab-scroll", id: "narr-scroll",
                            NarrativeTab { conv }
                        }
                    }
                }
            }
        }
    }
}
