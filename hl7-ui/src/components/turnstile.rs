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

//! Cloudflare Turnstile gate. Renders the widget; on solve it POSTs the token
//! to /api/verify (which sets the signed clearance cookie) and flips
//! `gate_cleared`, unlocking definition loading. Only mounted on web when the
//! server reports the gate is required.

use crate::state::AppState;
use dioxus::prelude::*;

#[component]
pub fn TurnstileGate() -> Element {
    let mut state = AppState::use_ctx();
    let sitekey = state.turnstile_sitekey.read().clone();

    // One-shot: inject the Turnstile script, render the widget into the
    // container below, and await the verify result from its callback.
    use_future(move || {
        let sitekey = sitekey.clone();
        async move {
            #[cfg(not(target_arch = "wasm32"))]
            let _ = &sitekey; // used only by the wasm block below
            #[cfg(target_arch = "wasm32")]
            {
                let key = serde_json::to_string(&sitekey).unwrap_or_else(|_| "\"\"".into());
                let script = format!(
                    r#"(function() {{
                        if (!window.__hl7_ts_loading) {{
                            window.__hl7_ts_loading = true;
                            var s = document.createElement('script');
                            s.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
                            s.async = true; s.defer = true;
                            document.head.appendChild(s);
                        }}
                        function ready(cb) {{
                            if (window.turnstile) cb();
                            else setTimeout(function() {{ ready(cb); }}, 100);
                        }}
                        ready(function() {{
                            var el = document.getElementById('turnstile-widget');
                            if (!el || el.dataset.rendered) return;
                            el.dataset.rendered = '1';
                            window.turnstile.render('#turnstile-widget', {{
                                sitekey: {key},
                                callback: async function(token) {{
                                    try {{
                                        var r = await fetch('/api/verify', {{
                                            method: 'POST',
                                            headers: {{ 'content-type': 'application/json' }},
                                            credentials: 'same-origin',
                                            body: JSON.stringify({{ token: token }})
                                        }});
                                        dioxus.send(r.ok);
                                    }} catch (e) {{ dioxus.send(false); }}
                                }}
                            }});
                        }});
                    }})();"#
                );
                let mut eval = document::eval(&script);
                if let Ok(true) = eval.recv::<bool>().await {
                    state.gate_cleared.set(true);
                }
            }
        }
    });

    rsx! {
        div { class: "turnstile-card",
            p { class: "turnstile-label",
                "Verify you're human to load the HL7 definitions."
            }
            div { id: "turnstile-widget" }
        }
    }
}
