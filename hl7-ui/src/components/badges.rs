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

//! Severity badge (issues list) and the segment flag pill.

use dioxus::prelude::*;

#[component]
pub fn SeverityBadge(severity: String) -> Element {
    let class = match severity.as_str() {
        "error" => "badge -error",
        "warning" => "badge -warning",
        _ => "badge -info",
    };
    rsx! {
        span { class: "{class}", "{severity}" }
    }
}
