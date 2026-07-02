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

//! Conformance issues. The engine is lenient: problems are reported here,
//! never by refusing to produce output.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub severity: Severity,
    /// Where in the message, HL7-style: "PID[1]-3[2].4" (segment[instance]-field[rep].component).
    pub location: String,
    pub message: String,
}

impl Issue {
    pub fn new(
        severity: Severity,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Issue {
            severity,
            location: location.into(),
            message: message.into(),
        }
    }
}
