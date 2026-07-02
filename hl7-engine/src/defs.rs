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

//! Definition model: the normalized HL7 specification data the engine interprets.
//!
//! One `Definitions` value per HL7 version, produced by `hl7-defs-etl` and
//! snapshotted under `defs/`. The engine is a generic interpreter over this
//! data - adding a version is a data drop, not a code change.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definitions {
    pub version: String,
    /// Trigger events (e.g. "ADT_A01", "ORU_R01") -> their message structure.
    pub events: BTreeMap<String, MessageEvent>,
    pub segments: BTreeMap<String, SegmentDef>,
    #[serde(rename = "dataTypes")]
    pub data_types: BTreeMap<String, DataTypeDef>,
    pub tables: BTreeMap<String, TableDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEvent {
    /// The structure this event uses; several events can share one (ADT_A04 -> ADT_A01).
    #[serde(rename = "msgStructId")]
    pub msg_struct_id: String,
    pub description: String,
    pub structure: Vec<StructureItem>,
}

/// One slot in a message structure: a concrete segment or a named group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StructureItem {
    Group {
        group: String,
        usage: String,
        rpt: String,
        children: Vec<StructureItem>,
    },
    Segment {
        segment: String,
        usage: String,
        rpt: String,
    },
}

impl StructureItem {
    pub fn usage(&self) -> &str {
        match self {
            StructureItem::Group { usage, .. } | StructureItem::Segment { usage, .. } => usage,
        }
    }

    pub fn repeats(&self) -> bool {
        let rpt = match self {
            StructureItem::Group { rpt, .. } | StructureItem::Segment { rpt, .. } => rpt,
        };
        rpt != "1"
    }

    pub fn required(&self) -> bool {
        self.usage() == "R"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

/// A segment field or a data type component - same shape in the spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub usage: String,
    pub rpt: String,
    pub length: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub table: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTypeDef {
    pub name: String,
    /// Empty for primitives (ST, NM, ID, …).
    pub components: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDef {
    pub name: String,
    /// "HL7" or "User" - User tables are site-definable and validate leniently.
    #[serde(rename = "tableType")]
    pub table_type: String,
    pub values: BTreeMap<String, String>,
}

impl Definitions {
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
