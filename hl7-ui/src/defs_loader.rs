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

//! Definition-snapshot loading. The only platform-gated module: web fetches
//! bundled assets over HTTP; desktop prefers an HL7_DEFS_DIR directory on
//! disk and falls back to the bundled assets.

use dioxus::prelude::*;

include!(concat!(env!("OUT_DIR"), "/defs_manifest.rs"));

const DEFS_DIR: Asset = asset!("/assets/defs");

/// Versions available without touching the network/disk: parsed from the
/// build-time manifest ("hl7-2.5.1.json" -> "2.5.1"). Desktop with
/// HL7_DEFS_DIR set re-scans that directory instead.
pub fn available_versions() -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dir) = std::env::var_os("HL7_DEFS_DIR") {
        let mut versions: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|e| version_of(&e.file_name().to_string_lossy()))
                    .collect()
            })
            .unwrap_or_default();
        versions.sort();
        return versions;
    }
    DEF_FILES.iter().filter_map(|f| version_of(f)).collect()
}

fn version_of(file: &str) -> Option<String> {
    file.strip_prefix("hl7-")?
        .strip_suffix(".json")
        .map(str::to_string)
}

pub fn file_of(version: &str) -> String {
    format!("hl7-{version}.json")
}

/// Load one snapshot's bytes.
pub async fn load_bytes(file: &str) -> Result<Vec<u8>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dir) = std::env::var_os("HL7_DEFS_DIR") {
        let path = std::path::Path::new(&dir).join(file);
        return std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()));
    }
    dioxus::asset_resolver::read_asset_bytes(&format!("{DEFS_DIR}/{file}"))
        .await
        .map_err(|e| format!("{file}: {e}"))
}
