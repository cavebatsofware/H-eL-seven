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

//! Application state: one context-provided bundle of signals.

use crate::convert::Converted;
use dioxus::prelude::*;
use hl7_engine::defs::Definitions;
use hl7_engine::Engine;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

pub const COLORWAYS: &[&str] = &[
    "forest", "warm", "plum", "avernus", "mineral", "daltonia", "tritan", "achroma",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Input,
    Explorer,
    Defs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Hl7,
    Json,
    Narrative,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DefsTab {
    Events,
    Segments,
    DataTypes,
    Tables,
}

/// Definitions-explorer state.
#[derive(Clone)]
pub struct DefsState {
    pub version: Option<String>,
    pub tab: DefsTab,
    /// Selected id within the active tab (event code, segment id, …).
    pub selected: Option<String>,
    pub filter: String,
}

impl Default for DefsState {
    fn default() -> Self {
        DefsState {
            version: None,
            tab: DefsTab::Events,
            selected: None,
            filter: String::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub view: Signal<View>,
    pub tab: Signal<Tab>,
    pub input: Signal<String>,
    pub converted: Signal<Option<Rc<Converted>>>,
    pub sel_seg: Signal<Option<String>>,
    /// Collapsed JSON node ids. `None` = default (every collapsible depth ≥ 1).
    pub collapsed: Signal<Option<HashSet<String>>>,
    pub copied: Signal<bool>,
    /// (colorway, dark)
    pub theme: Signal<(String, bool)>,
    pub defs_cache: Signal<BTreeMap<String, Rc<Definitions>>>,
    pub engine: Signal<Option<Rc<Engine>>>,
    pub defs_state: Signal<DefsState>,
    pub convert_error: Signal<Option<String>>,
    pub converting: Signal<bool>,
    /// Counter mixed into auto-created seeds (uniqueness within one clock tick).
    pub sample_counter: Signal<u64>,
    /// Seed field: used verbatim when filled; auto-created and displayed here
    /// when empty, so every sample is reproducible.
    pub seed_input: Signal<String>,
    /// Fraction of generated samples that get an injected defect, 0.0..=1.0
    /// (hl7-gen --messy). 0 = always clean, 1 = every sample defective.
    pub defect_rate: Signal<f64>,
    /// Defect report for the last generated sample (empty = clean).
    pub gen_report: Signal<Vec<String>>,
    /// Turnstile gate: `/api/config` has been consulted.
    pub gate_ready: Signal<bool>,
    /// A challenge is required before the defs can be fetched.
    pub gate_required: Signal<bool>,
    /// The challenge has been solved (or is not required).
    pub gate_cleared: Signal<bool>,
    /// Public Turnstile site key from `/api/config`.
    pub turnstile_sitekey: Signal<String>,
}

impl AppState {
    pub fn provide() -> Self {
        let state = AppState {
            view: Signal::new(View::Input),
            tab: Signal::new(Tab::Hl7),
            input: Signal::new(String::new()),
            converted: Signal::new(None),
            sel_seg: Signal::new(None),
            collapsed: Signal::new(None),
            copied: Signal::new(false),
            theme: Signal::new(("forest".to_string(), false)),
            defs_cache: Signal::new(BTreeMap::new()),
            engine: Signal::new(None),
            defs_state: Signal::new(DefsState::default()),
            convert_error: Signal::new(None),
            converting: Signal::new(false),
            sample_counter: Signal::new(0),
            seed_input: Signal::new(String::new()),
            defect_rate: Signal::new(0.0),
            gen_report: Signal::new(Vec::new()),
            // Optimistic defaults: no gate until /api/config says otherwise.
            gate_ready: Signal::new(false),
            gate_required: Signal::new(false),
            gate_cleared: Signal::new(true),
            turnstile_sitekey: Signal::new(String::new()),
        };
        use_context_provider(|| state)
    }

    pub fn use_ctx() -> Self {
        use_context::<AppState>()
    }

    /// Consult the server for the Turnstile gate. On web this hits
    /// `/api/config`; a missing endpoint (e.g. `dx serve`, or the desktop
    /// build with no server) leaves the gate disabled.
    pub async fn init_gate(mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(text) = crate::js::fetch_text("/api/config").await {
                if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&text) {
                    let enabled = cfg["gateEnabled"].as_bool().unwrap_or(false);
                    let sitekey = cfg["turnstileSitekey"].as_str().unwrap_or("").to_string();
                    let required = enabled && !sitekey.is_empty();
                    self.turnstile_sitekey.set(sitekey);
                    self.gate_required.set(required);
                    self.gate_cleared.set(!required);
                }
            }
        }
        self.gate_ready.set(true);
    }

    /// Ensure the snapshot for `hint` (or all snapshots when unknown) is in
    /// the cache, then rebuild the engine from everything cached. The engine
    /// handles closest-version fallback among whatever is loaded.
    pub async fn ensure_defs(mut self, hint: Option<String>) -> Result<(), String> {
        let available = crate::defs_loader::available_versions();
        if available.is_empty() {
            return Err(
                "No definition snapshots found. Run hl7-defs-etl to regenerate defs/ \
                 and rebuild."
                    .to_string(),
            );
        }
        let targets: Vec<String> = match hint {
            Some(v) if available.contains(&v) => vec![v],
            _ => available,
        };
        let mut grew = false;
        for version in targets {
            if self.defs_cache.read().contains_key(&version) {
                continue;
            }
            let bytes =
                crate::defs_loader::load_bytes(&crate::defs_loader::file_of(&version)).await?;
            let defs = Definitions::from_json(&bytes).map_err(|e| format!("{version}: {e}"))?;
            self.defs_cache.write().insert(version, Rc::new(defs));
            grew = true;
        }
        if grew || self.engine.read().is_none() {
            let mut engine = Engine::new();
            for defs in self.defs_cache.read().values() {
                engine.add((**defs).clone());
            }
            self.engine.set(Some(Rc::new(engine)));
        }
        Ok(())
    }

    /// Definitions for generating a sample: 2.5.1 if bundled, else the
    /// newest available.
    async fn sample_defs(self) -> Result<Rc<Definitions>, String> {
        let available = crate::defs_loader::available_versions();
        let version = if available.iter().any(|v| v == "2.5.1") {
            "2.5.1".to_string()
        } else {
            available
                .last()
                .cloned()
                .ok_or("No definition snapshots found. Run hl7-defs-etl to regenerate defs/.")?
        };
        self.ensure_defs(Some(version.clone())).await?;
        Ok(self.defs_cache.read()[&version].clone())
    }

    /// A fresh seed from the clock, mixed with a counter so rapid clicks
    /// within one tick still differ.
    fn fresh_seed(mut self) -> u64 {
        let n = *self.sample_counter.read();
        self.sample_counter.set(n + 1);
        #[cfg(target_arch = "wasm32")]
        let entropy = js_sys::Date::now() as u64;
        #[cfg(not(target_arch = "wasm32"))]
        let entropy = {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            now.as_secs() ^ u64::from(now.subsec_nanos()).wrapping_mul(0x9E37_79B9)
        };
        entropy ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }

    /// The seed to generate with: the field's value when filled (must be a
    /// non-negative integer), otherwise a fresh one.
    fn resolve_seed(self) -> Result<u64, String> {
        let typed = self.seed_input.read().trim().to_string();
        if typed.is_empty() {
            return Ok(self.fresh_seed());
        }
        typed
            .parse::<u64>()
            .map_err(|_| format!("invalid seed \"{typed}\": expected a non-negative integer"))
    }

    /// Generate a message with hl7-gen -> returns the text, LF-separated for
    /// the textarea. The seed comes from the seed field (or is created) and
    /// is written back to the field so the sample is reproducible. The defect
    /// rate is passed straight through as `--messy`, so at 1.0 every sample is
    /// defective and the report below the editor always says what to expect;
    /// the injected-defect report lands in `gen_report`.
    async fn generate_sample_text(mut self) -> Result<String, String> {
        let defs = self.sample_defs().await?;
        let seed = self.resolve_seed()?;
        let events = hl7_gen::Generator::default_events(&defs);
        if events.is_empty() {
            return Err("definitions expose no generatable trigger events".to_string());
        }
        let event = &events[(seed as usize) % events.len()];
        let config = hl7_gen::Config {
            messy: *self.defect_rate.read(),
            ..hl7_gen::Config::default()
        };
        let mut generator = hl7_gen::Generator::new(&defs, seed, config);
        let generated = generator
            .generate(event)
            .ok_or_else(|| format!("cannot generate {event}"))?;
        self.gen_report.set(generated.defects);
        self.seed_input.set(seed.to_string());
        Ok(generated.text.trim_end_matches('\r').replace('\r', "\n"))
    }

    pub fn generate_sample(mut self) {
        spawn(async move {
            match self.generate_sample_text().await {
                Ok(text) => {
                    self.input.set(text);
                    self.convert_error.set(None);
                }
                Err(e) => self.convert_error.set(Some(e)),
            }
        });
    }

    pub fn convert(mut self) {
        spawn(async move {
            self.converting.set(true);
            self.convert_error.set(None);
            let result = self.convert_inner().await;
            if let Err(e) = result {
                self.convert_error.set(Some(e));
            }
            self.converting.set(false);
        });
    }

    async fn convert_inner(mut self) -> Result<(), String> {
        if self.input.read().trim().is_empty() {
            let text = self.generate_sample_text().await?;
            self.input.set(text);
        }
        let text = self.input.read().clone();
        self.ensure_defs(crate::convert::sniff_msh12(&text)).await?;
        let engine = self
            .engine
            .read()
            .clone()
            .expect("engine after ensure_defs");
        let doc = engine.translate(&text).map_err(|e| e.to_string())?;
        let defs_version = doc["_meta"]["definitionsVersion"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let defs = self.defs_cache.read().get(&defs_version).cloned();
        let converted = crate::convert::build(&text, doc, defs.as_deref());
        self.converted.set(Some(Rc::new(converted)));
        self.view.set(View::Explorer);
        self.tab.set(Tab::Hl7);
        self.sel_seg.set(None);
        self.collapsed.set(None);
        self.copied.set(false);
        Ok(())
    }

    pub fn new_message(mut self) {
        self.view.set(View::Input);
        self.sel_seg.set(None);
        self.input.set(String::new());
        self.seed_input.set(String::new());
        self.gen_report.set(Vec::new());
        self.convert_error.set(None);
    }

    /// Current collapsed set, materializing the default (all depth ≥ 1).
    fn collapsed_or_default(&self, conv: &Converted) -> HashSet<String> {
        self.collapsed
            .read()
            .clone()
            .unwrap_or_else(|| crate::jsontree::default_collapsed(&conv.nodes))
    }

    /// Select a segment: expand its JSON node (and every ancestor), then
    /// scroll the active tab to it.
    pub fn select_seg(mut self, code: &str) {
        let Some(conv) = self.converted.read().clone() else {
            return;
        };
        let node_id = conv.seg_occurrences.get(&(code.to_string(), 1)).cloned();
        if let Some(node_id) = &node_id {
            let mut collapsed = self.collapsed_or_default(&conv);
            for id in crate::segmap::with_ancestors(node_id) {
                collapsed.remove(&id);
            }
            self.collapsed.set(Some(collapsed));
        }
        self.sel_seg.set(Some(code.to_string()));
        match *self.tab.read() {
            Tab::Json => {
                if let Some(node_id) = &node_id {
                    crate::js::scroll_to("json-scroll", &format!("json-{node_id}"));
                }
            }
            Tab::Hl7 => crate::js::scroll_to("hl7-scroll", &format!("hl7-{code}-1")),
            Tab::Narrative => {}
        }
    }

    /// Issue / unmapped click: switch to the HL7 tab and scroll to the
    /// exact source line.
    pub fn jump_to(mut self, code: &str, occ: usize) {
        let Some(conv) = self.converted.read().clone() else {
            return;
        };
        if let Some(node_id) = conv.seg_occurrences.get(&(code.to_string(), occ)) {
            let mut collapsed = self.collapsed_or_default(&conv);
            for id in crate::segmap::with_ancestors(node_id) {
                collapsed.remove(&id);
            }
            self.collapsed.set(Some(collapsed));
        }
        self.sel_seg.set(Some(code.to_string()));
        self.tab.set(Tab::Hl7);
        crate::js::scroll_to("hl7-scroll", &format!("hl7-{code}-{occ}"));
    }
}
