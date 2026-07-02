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

//! H-eL-seven: schema-aware HL7 v2 -> JSON translation.
//!
//! Pipeline: syntactic parse -> structure select (MSH-9/MSH-12) -> group bind ->
//! typed decode + validate -> JSON document. Lenient throughout: conformance
//! problems become `issues[]` entries, never rejections; the only hard errors
//! are messages whose delimiters cannot even be read.

pub mod bind;
pub mod decode;
pub mod defs;
pub mod escape;
pub mod issue;
pub mod json;
pub mod syntax;

use defs::Definitions;
use issue::{Issue, Severity};
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum EngineError {
    Parse(syntax::ParseError),
    /// No definition snapshots loaded.
    NoDefinitions,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Parse(e) => write!(f, "{e}"),
            EngineError::NoDefinitions => write!(f, "no HL7 definition snapshots loaded"),
        }
    }
}

impl std::error::Error for EngineError {}

pub struct Engine {
    /// version tuple (major, minor, patch) -> definitions
    versions: BTreeMap<(u32, u32, u32), Definitions>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            versions: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, defs: Definitions) {
        self.versions.insert(parse_version(&defs.version), defs);
    }

    /// Load every defs/hl7-*.json snapshot in a directory.
    pub fn load_dir(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let mut engine = Engine::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("hl7-") && name.ends_with(".json") {
                let bytes = std::fs::read(entry.path())?;
                let defs = Definitions::from_json(&bytes)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                engine.add(defs);
            }
        }
        Ok(engine)
    }

    /// Translate one HL7 message to its JSON document.
    pub fn translate(&self, text: &str) -> Result<serde_json::Value, EngineError> {
        let msg = syntax::parse(text).map_err(EngineError::Parse)?;
        let mut issues: Vec<Issue> = Vec::new();

        let requested = msg.segments[0]
            .field(12)
            .and_then(|f| f.repeats.first())
            .and_then(|r| r.components.first())
            .and_then(|c| c.subcomponents.first())
            .copied()
            .unwrap_or("");
        let defs = self.resolve_version(requested, &mut issues)?;

        let (selection, mut select_issues) = bind::select_event(defs, &msg.segments[0]);
        issues.append(&mut select_issues);

        Ok(match selection {
            Some((key, event)) => {
                let bound = bind::bind(&msg, &key, event, &mut issues);
                json::emit(defs, &msg, &bound, &mut issues)
            }
            None => json::emit_unbound(defs, &msg, &mut issues),
        })
    }

    /// Exact version if loaded; otherwise the closest available (preferring
    /// the greatest loaded version ≤ the requested one), with a warning.
    fn resolve_version(
        &self,
        requested: &str,
        issues: &mut Vec<Issue>,
    ) -> Result<&Definitions, EngineError> {
        if self.versions.is_empty() {
            return Err(EngineError::NoDefinitions);
        }
        if requested.is_empty() {
            let (_, defs) = self.versions.iter().next_back().unwrap();
            issues.push(Issue::new(
                Severity::Warning,
                "MSH-12",
                format!("no version declared; using HL7 v{}", defs.version),
            ));
            return Ok(defs);
        }
        let want = parse_version(requested);
        if let Some(defs) = self.versions.get(&want) {
            return Ok(defs);
        }
        let defs = self
            .versions
            .range(..=want)
            .next_back()
            .map(|(_, d)| d)
            .unwrap_or_else(|| self.versions.values().next().unwrap());
        issues.push(Issue::new(
            Severity::Warning,
            "MSH-12",
            format!(
                "HL7 v{requested} definitions not loaded; using v{}",
                defs.version
            ),
        ));
        Ok(defs)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_version(v: &str) -> (u32, u32, u32) {
    let mut parts = v.split('.').map(|p| p.trim().parse::<u32>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}
