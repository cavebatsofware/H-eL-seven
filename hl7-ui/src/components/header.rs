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

//! App header: wordmark, view nav, explorer message title + New message,
//! theme picker (colorway Select + dark toggle).

use crate::components::select::{Select, SelectOption};
use crate::state::{AppState, View, COLORWAYS};
use dioxus::prelude::*;

/// Author's site.
const PROFILE_URL: &str = "https://dev.cavebatsoftware.com";
/// Project source. TODO(confirm): no git remote is set; verify this URL.
const GITHUB_URL: &str = "https://github.com/cavebatsofware/H-eL-seven";

#[component]
pub fn Header() -> Element {
    let state = AppState::use_ctx();
    let view = *state.view.read();
    let (colorway, dark) = state.theme.read().clone();

    let msg_title = state
        .converted
        .read()
        .as_ref()
        .map(|c| format!("{} · {}", c.meta.event, c.meta.control_id));

    rsx! {
        div { class: "app-header",
            div { class: "wordmark",
                span { class: "wm-strong", "HL7" }
                span { class: "wm-arrow", "→" }
                span { class: "wm-strong", "JSON" }
                span { class: "wm-eyebrow", "visualizer" }
            }
            button {
                class: if view != View::Defs { "nav-btn active" } else { "nav-btn" },
                onclick: move |_| {
                    let mut state = state;
                    if *state.view.read() == View::Defs {
                        let target = if state.converted.read().is_some() {
                            View::Explorer
                        } else {
                            View::Input
                        };
                        state.view.set(target);
                    }
                },
                "Convert"
            }
            button {
                class: if view == View::Defs { "nav-btn active" } else { "nav-btn" },
                onclick: move |_| {
                    let mut state = state;
                    state.view.set(View::Defs);
                },
                "Definitions"
            }
            div { class: "spacer" }
            if view == View::Explorer {
                div { class: "header-right",
                    if let Some(title) = msg_title {
                        span { class: "msg-title", "{title}" }
                    }
                    button {
                        class: "btn-outline",
                        onclick: move |_| state.new_message(),
                        "New message"
                    }
                }
            }
            Select::<String> {
                aria_label: "Colorway",
                default_value: Some(colorway.clone()),
                on_value_change: move |value: Option<String>| {
                    let mut state = state;
                    if let Some(colorway) = value {
                        let dark = state.theme.read().1;
                        state.theme.set((colorway, dark));
                    }
                },
                for (i, name) in COLORWAYS.iter().enumerate() {
                    SelectOption::<String> {
                        index: i,
                        value: name.to_string(),
                        text_value: name.to_string(),
                        "{name}"
                    }
                }
            }
            button {
                class: "btn-outline",
                title: "Toggle dark mode",
                onclick: move |_| {
                    let mut state = state;
                    let (colorway, dark) = state.theme.read().clone();
                    state.theme.set((colorway, !dark));
                },
                if dark { "☾" } else { "☀" }
            }
            div { class: "header-links",
                a {
                    class: "header-link",
                    href: PROFILE_URL,
                    target: "_blank",
                    rel: "noopener noreferrer",
                    title: "Grant DeFayette · dev.cavebatsoftware.com",
                    "cavebatsoftware ↗"
                }
                a {
                    class: "header-link icon",
                    href: GITHUB_URL,
                    target: "_blank",
                    rel: "noopener noreferrer",
                    title: "Source on GitHub",
                    aria_label: "Source on GitHub",
                    // GitHub mark (inline SVG, no icon dependency).
                    svg {
                        width: "18",
                        height: "18",
                        view_box: "0 0 16 16",
                        fill: "currentColor",
                        "aria-hidden": "true",
                        path {
                            d: "M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 \
                                0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 \
                                1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 \
                                0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 \
                                2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 \
                                1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 \
                                2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z",
                        }
                    }
                }
            }
        }
    }
}
