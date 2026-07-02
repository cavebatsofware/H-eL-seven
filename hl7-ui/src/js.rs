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

//! Small JS interop helpers via document::eval. The design mandates
//! scrollTop math (never scrollIntoView) and data-theme on <html>.
//!
//! Safety: document::eval is Dioxus's own JS-interop channel running fixed
//! scripts we author here - not JS `eval()` over user input. Every dynamic
//! value is embedded as a JSON string literal (js_str), never spliced raw.

use dioxus::document;

fn js_str(s: &str) -> String {
    serde_json::to_string(s).expect("string serializes")
}

/// Scroll `container_id` so `target_id` sits ~60px below its top.
/// Double-rAF so the scroll runs after Dioxus flushes DOM changes made in
/// the same tick (tab switch + selection).
pub fn scroll_to(container_id: &str, target_id: &str) {
    let code = format!(
        "requestAnimationFrame(() => requestAnimationFrame(() => {{\n\
           const el = document.getElementById({t});\n\
           const sc = document.getElementById({c});\n\
           if (el && sc) sc.scrollTop = Math.max(0, el.offsetTop - sc.offsetTop - 60);\n\
         }}));",
        t = js_str(target_id),
        c = js_str(container_id)
    );
    document::eval(&code);
}

/// Set the Riposte colorway on <html data-theme="…">.
pub fn set_theme(colorway: &str, dark: bool) {
    let name = if dark {
        format!("{colorway}-dark")
    } else {
        colorway.to_string()
    };
    document::eval(&format!(
        "document.documentElement.setAttribute('data-theme', {});",
        js_str(&name)
    ));
}

/// Select an input's entire contents (so typing replaces the old value).
pub fn select_all(id: &str) {
    document::eval(&format!(
        "const el = document.getElementById({}); if (el) el.select();",
        js_str(id)
    ));
}

/// Copy text to the clipboard. Works on web and WebView2/WKWebView;
/// may silently no-op on Linux webkitgtk (acceptable for the dev tool).
pub fn copy_text(text: &str) {
    document::eval(&format!("navigator.clipboard.writeText({});", js_str(text)));
}

/// Cross-platform sleep without an async runtime: resolves after `ms`.
pub async fn sleep_ms(ms: u32) {
    let mut eval = document::eval(&format!(
        "await new Promise(r => setTimeout(r, {ms})); dioxus.send(true);"
    ));
    let _ = eval.recv::<bool>().await;
}

/// GET `url` (same-origin) and return the response body, or `None` on any
/// error (network failure, missing endpoint). Web only.
#[cfg(target_arch = "wasm32")]
pub async fn fetch_text(url: &str) -> Option<String> {
    let mut eval = document::eval(&format!(
        "try {{ const r = await fetch({}, {{ credentials: 'same-origin' }}); \
           dioxus.send(r.ok ? await r.text() : ''); }} \
         catch (e) {{ dioxus.send(''); }}",
        js_str(url)
    ));
    match eval.recv::<String>().await {
        Ok(text) if !text.is_empty() => Some(text),
        _ => None,
    }
}
