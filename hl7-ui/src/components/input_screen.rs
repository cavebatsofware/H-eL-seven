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

//! Input screen: paste an HL7 v2 message, generate a sample, convert.

use crate::components::turnstile::TurnstileGate;
use crate::defs_loader;
use crate::state::AppState;
use dioxus::prelude::*;

#[component]
pub fn InputScreen() -> Element {
    let mut state = AppState::use_ctx();
    let converting = *state.converting.read();
    let error = state.convert_error.read().clone();
    let no_defs = defs_loader::available_versions().is_empty();
    let messy = *state.messy.read();
    let gen_report = state.gen_report.read().clone();

    // Turnstile gate: block conversion until the challenge is solved (or the
    // server reports no gate). `gate_ready` guards the brief window before
    // /api/config has answered.
    let gate_ready = *state.gate_ready.read();
    let needs_challenge = *state.gate_required.read() && !*state.gate_cleared.read();
    let convert_blocked = converting || no_defs || !gate_ready || needs_challenge;

    rsx! {
        div { class: "input-scroll",
            div { class: "input-col",
                div { class: "eyebrow", "Convert" }
                h1 { class: "page-title", "HL7 v2 to annotated JSON" }
                p { class: "intro",
                    "Paste an HL7 v2.x message below, then convert. Inspect the raw \
                     segments, the mapped JSON, and a generated narrative, with every \
                     unmapped field and spec deviation flagged."
                }
                div { class: "editor-card",
                    div { class: "editor-head",
                        span { class: "editor-name", "message.hl7" }
                        div { class: "editor-actions",
                            label {
                                class: "toggle",
                                title: "Inject defects into ~1 of 3 generated samples \
                                        (hl7gen --messy); the validator must flag them",
                                input {
                                    r#type: "checkbox",
                                    checked: messy,
                                    onchange: move |e| state.messy.set(e.checked()),
                                }
                                "Inject defects"
                            }
                            // Deliberately NOT a <label> wrapper: a label
                            // click re-focuses the input and collapses any
                            // text selection, so select-then-type appends
                            // instead of replacing.
                            span {
                                class: "toggle",
                                title: "Sample seed: reused as long as it stays here; \
                                        clear the field to get a fresh one",
                                "seed"
                            }
                            input {
                                id: "seed-input",
                                class: "seed-input",
                                r#type: "text",
                                inputmode: "numeric",
                                placeholder: "auto",
                                title: "Sample seed: reused as long as it stays here; \
                                        clear the field to get a fresh one",
                                value: "{state.seed_input}",
                                // URL-bar semantics: a seed is replaced, copied,
                                // or cleared. Select all on focus AND click so
                                // typing always replaces (clicks past the text
                                // land no selection otherwise, and typing would append).
                                onfocus: move |_| crate::js::select_all("seed-input"),
                                onclick: move |_| crate::js::select_all("seed-input"),
                                oninput: move |e| state.seed_input.set(e.value()),
                            }
                            button {
                                class: "btn-text",
                                onclick: move |_| state.generate_sample(),
                                "Generate sample →"
                            }
                        }
                    }
                    textarea {
                        class: "editor",
                        spellcheck: false,
                        placeholder: "MSH|^~\\&|SENDING_APP|SENDING_FAC|...",
                        value: "{state.input}",
                        oninput: move |e| {
                            state.input.set(e.value());
                            // The defect report describes a generated sample;
                            // it no longer applies once the text is edited.
                            state.gen_report.set(Vec::new());
                        },
                    }
                }
                if needs_challenge {
                    TurnstileGate {}
                }
                div { class: "action-row",
                    button {
                        class: "btn-primary",
                        disabled: convert_blocked,
                        onclick: move |_| state.convert(),
                        if converting { "Converting…" } else { "Convert message" }
                    }
                    span { class: "helper",
                        if needs_challenge {
                            "Complete the verification above to convert."
                        } else {
                            "Empty input converts a "
                            code { class: "chip", "hl7gen" }
                            " sample."
                        }
                    }
                }
                if !gen_report.is_empty() {
                    div { class: "notice-card",
                        strong { "Injected defects: " }
                        "{gen_report.join(\" · \")} - conversion should flag these."
                    }
                }
                if no_defs {
                    div { class: "notice-card",
                        "No definition snapshots bundled. Run hl7-defs-etl to regenerate \
                         defs/, then rebuild the app."
                    }
                }
                if let Some(error) = error {
                    div { class: "error-card", "{error}" }
                }

                footer { class: "disclaimer",
                    p {
                        strong { "Site use: non-commercial and educational purposes only." }
                        " This hosted instance is a demonstration of HL7 v2 to JSON \
                         conversion. It is not for clinical care, diagnosis, or real \
                         patient information, and is provided without warranty."
                    }
                    p {
                        "Source code © 2026 CavebatSoftware LLC - Grant DeFayette "
                        a {
                            class: "disclaimer-link",
                            href: "https://www.gnu.org/licenses/gpl-3.0.html",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "GPL-3.0-only"
                        }
                        ". The code license is separate from these site terms and is not \
                         restricted by them."
                    }
                }
            }
        }
    }
}
