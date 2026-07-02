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

//! hl7-ui - HL7 v2 -> JSON visualizer (Dioxus; web primary, desktop shared).

mod components;
mod convert;
mod defs_loader;
mod js;
mod jsontree;
mod narrative;
mod segmap;
mod state;

use components::defs_explorer::DefsExplorer;
use components::explorer::Explorer;
use components::header::Header;
use components::input_screen::InputScreen;
use dioxus::prelude::*;
use state::{AppState, View};

fn main() {
    // webkitgtk's DMA-BUF renderer is broken on NVIDIA (tiny text, collapsed
    // layout - DioxusLabs/dioxus#3427); disable it unless the caller has
    // already chosen a setting.
    #[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let state = AppState::provide();

    // Keep <html data-theme> in sync with the theme signal.
    use_effect(move || {
        let (colorway, dark) = state.theme.read().clone();
        js::set_theme(&colorway, dark);
    });

    // Consult the server once for the Turnstile gate (no-op off web).
    use_future(move || async move { state.init_gate().await });

    // Vendored fonts, wired from Rust: asset! hashes the filenames, so the
    // resolved URLs are formatted into an @font-face block here rather than
    // referenced by a fixed url() inside a bundled CSS file. Weight 400-700
    // variable, latin subset. Without these the text fonts fall back to the
    // system, and digits get hijacked by Noto Color Emoji (colorful glyphs).
    const INTER: Asset = asset!("/assets/fonts/Inter-latin.woff2");
    const MONO: Asset = asset!("/assets/fonts/JetBrainsMono-latin.woff2");
    let font_faces = format!(
        "@font-face{{font-family:'Inter';font-style:normal;font-weight:400 700;\
         font-display:swap;src:url({INTER}) format('woff2');}}\
         @font-face{{font-family:'JetBrains Mono';font-style:normal;font-weight:400 700;\
         font-display:swap;src:url({MONO}) format('woff2');}}"
    );

    let view = *state.view.read();
    rsx! {
        document::Style { {font_faces} }
        document::Stylesheet { href: asset!("/assets/riposte-tokens.css") }
        document::Stylesheet { href: asset!("/assets/app.css") }
        document::Title { "HL7 to JSON Visualizer" }
        div { class: "app-root",
            Header {}
            match view {
                View::Input => rsx! { InputScreen {} },
                View::Explorer if state.converted.read().is_some() => rsx! { Explorer {} },
                View::Explorer => rsx! { InputScreen {} },
                View::Defs => rsx! { DefsExplorer {} },
            }
        }
    }
}
