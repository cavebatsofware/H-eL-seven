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

//! Sync definitions from ../defs into assets/defs so the asset
//! bundler can ship them, and generate a manifest of what is available.
//!
//! The defs are generated with hl7-defs-etl; the build must
//! succeed with zero defs present.

use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let src_dir = Path::new(&manifest_dir).join("../defs");
    let dst_dir = Path::new(&manifest_dir).join("assets/defs");

    println!("cargo:rerun-if-changed={}", src_dir.display());
    println!("cargo:rerun-if-changed={}", dst_dir.display());

    std::fs::create_dir_all(&dst_dir).expect("create assets/defs");

    // Copy hl7-*.json from ../defs (if present), skipping up-to-date files.
    if let Ok(entries) = std::fs::read_dir(&src_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !(name_str.starts_with("hl7-") && name_str.ends_with(".json")) {
                continue;
            }
            let dst = dst_dir.join(&name);
            let stale = match (entry.metadata(), dst.metadata()) {
                (Ok(s), Ok(d)) => match (s.modified(), d.modified()) {
                    (Ok(sm), Ok(dm)) => sm > dm,
                    _ => true,
                },
                _ => true,
            };
            if stale {
                std::fs::copy(entry.path(), &dst)
                    .unwrap_or_else(|e| panic!("copy {name_str}: {e}"));
            }
        }
    }

    // Manifest: every def now present under assets/defs, sorted.
    let mut files: Vec<String> = std::fs::read_dir(&dst_dir)
        .expect("read assets/defs")
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name.starts_with("hl7-") && name.ends_with(".json")).then_some(name)
        })
        .collect();
    files.sort();

    let manifest = format!(
        "pub static DEF_FILES: &[&str] = &{:?};\n",
        files.iter().map(String::as_str).collect::<Vec<_>>()
    );
    std::fs::write(Path::new(&out_dir).join("defs_manifest.rs"), manifest)
        .expect("write defs_manifest.rs");
}
