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

//! Definitions explorer: pick a loaded HL7 version and browse its trigger
//! events (nested structure), segments, data types, and tables, with
//! cross-links (field -> data type -> table). Same design language as the
//! message explorer: 272px rail + tabbed main pane.

use crate::components::tabs::{TabContent, TabList, TabTrigger, Tabs};
use crate::defs_loader;
use crate::state::{AppState, DefsTab};
use dioxus::prelude::*;
use hl7_engine::defs::{Definitions, StructureItem};
use std::rc::Rc;

#[component]
pub fn DefsExplorer() -> Element {
    let mut state = AppState::use_ctx();
    let versions = defs_loader::available_versions();
    let ds = state.defs_state.read().clone();
    let defs: Option<Rc<Definitions>> = ds
        .version
        .as_ref()
        .and_then(|v| state.defs_cache.read().get(v).cloned());

    let tab_value = use_memo(move || {
        Some(
            match state.defs_state.read().tab {
                DefsTab::Events => "events",
                DefsTab::Segments => "segments",
                DefsTab::DataTypes => "datatypes",
                DefsTab::Tables => "tables",
            }
            .to_string(),
        )
    });

    rsx! {
        div { class: "explorer-grid",
            aside { class: "rail", id: "rail-scroll",
                div { class: "rail-head",
                    div { class: "rail-eyebrow", "Version" }
                    span { class: "rail-count", "{versions.len()} bundled" }
                }
                if versions.is_empty() {
                    div { class: "rail-section",
                        div { class: "issue-msg",
                            "No definition snapshots bundled. Run hl7-defs-etl to \
                             regenerate defs/, then rebuild."
                        }
                    }
                }
                for version in versions {
                    {
                        let v = version.clone();
                        let selected = ds.version.as_deref() == Some(version.as_str());
                        rsx! {
                            button {
                                class: if selected { "seg-btn selected" } else { "seg-btn" },
                                onclick: move |_| {
                                    let v = v.clone();
                                    let mut state = state;
                                    state.defs_state.write().version = Some(v.clone());
                                    state.defs_state.write().selected = None;
                                    spawn(async move {
                                        if let Err(e) = state.ensure_defs(Some(v)).await {
                                            state.convert_error.set(Some(e));
                                        }
                                    });
                                },
                                span { class: "seg-code", "v{version}" }
                                span { class: "seg-label", "HL7 {version}" }
                            }
                        }
                    }
                }
                if let Some(defs) = &defs {
                    div { class: "rail-divider later" }
                    div { class: "rail-section",
                        div { class: "rail-eyebrow", style: "margin-bottom: 8px;", "Contents" }
                        for (k, v) in [
                            ("Events", defs.events.len()),
                            ("Segments", defs.segments.len()),
                            ("Data types", defs.data_types.len()),
                            ("Tables", defs.tables.len()),
                        ] {
                            div { class: "meta-row",
                                span { class: "meta-k", "{k}" }
                                span { class: "meta-v", "{v}" }
                            }
                        }
                    }
                }
            }
            main { class: "main-pane",
                if defs.is_some() {
                    Tabs {
                        value: tab_value,
                        on_value_change: move |v: String| {
                            let tab = match v.as_str() {
                                "segments" => DefsTab::Segments,
                                "datatypes" => DefsTab::DataTypes,
                                "tables" => DefsTab::Tables,
                                _ => DefsTab::Events,
                            };
                            let mut ds = state.defs_state.write();
                            if ds.tab != tab {
                                ds.tab = tab;
                                ds.selected = None;
                                ds.filter = String::new();
                            }
                        },
                        TabList {
                            TabTrigger { value: "events", index: 0usize, "Events" }
                            TabTrigger { value: "segments", index: 1usize, "Segments" }
                            TabTrigger { value: "datatypes", index: 2usize, "Data types" }
                            TabTrigger { value: "tables", index: 3usize, "Tables" }
                        }
                        TabContent { value: "events", index: 0usize,
                            div { class: "tab-scroll",
                                EventsTab {}
                            }
                        }
                        TabContent { value: "segments", index: 1usize,
                            div { class: "tab-scroll",
                                SegmentsTab {}
                            }
                        }
                        TabContent { value: "datatypes", index: 2usize,
                            div { class: "tab-scroll",
                                DataTypesTab {}
                            }
                        }
                        TabContent { value: "tables", index: 3usize,
                            div { class: "tab-scroll",
                                TablesTab {}
                            }
                        }
                    }
                } else if ds.version.is_some() {
                    div { class: "pane-pad caption", "Loading definitions…" }
                } else {
                    div { class: "pane-pad caption", "Pick an HL7 version to browse its definitions." }
                }
            }
        }
    }
}

/// Filtered master list (left) + detail (right) inside a tab pane.
#[component]
fn MasterDetail(
    ids: Vec<(String, String)>, // (id, list label suffix)
    detail: Element,
) -> Element {
    let mut state = AppState::use_ctx();
    let ds = state.defs_state.read().clone();
    let filter = ds.filter.to_lowercase();
    let shown: Vec<&(String, String)> = ids
        .iter()
        .filter(|(id, label)| {
            filter.is_empty()
                || id.to_lowercase().contains(&filter)
                || label.to_lowercase().contains(&filter)
        })
        .collect();

    rsx! {
        div { class: "pane-pad", style: "display: grid; grid-template-columns: 240px 1fr; gap: 16px; align-items: start;",
            div {
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "filter…",
                    value: "{ds.filter}",
                    oninput: move |e| state.defs_state.write().filter = e.value(),
                }
                div { style: "max-height: none;",
                    for (id, label) in shown.iter().map(|(a, b)| (a.clone(), b.clone())) {
                        {
                            let sel_id = id.clone();
                            let selected = ds.selected.as_deref() == Some(id.as_str());
                            rsx! {
                                button {
                                    class: if selected { "seg-btn selected" } else { "seg-btn" },
                                    style: "width: 100%; margin: 1px 0;",
                                    onclick: move |_| {
                                        state.defs_state.write().selected = Some(sel_id.clone());
                                    },
                                    span { class: "seg-code", "{id}" }
                                    span { class: "seg-label", "{label}" }
                                }
                            }
                        }
                    }
                }
            }
            div { {detail} }
        }
    }
}

fn usage_chip(usage: &str) -> Element {
    let (class, label) = match usage {
        "R" => ("usage-chip -required", "R"),
        other => ("usage-chip", other),
    };
    rsx! {
        span { class: "{class}", title: "usage", "{label}" }
    }
}

fn rpt_chip(rpt: &str) -> Element {
    if rpt == "1" {
        return rsx! {};
    }
    rsx! {
        span { class: "usage-chip -repeat", title: "repeats", "⟳ {rpt}" }
    }
}

/// Definitions for the currently selected version, if loaded.
fn current_defs(state: &AppState) -> Option<Rc<Definitions>> {
    let version = state.defs_state.read().version.clone()?;
    state.defs_cache.read().get(&version).cloned()
}

/// Cross-link: jump to another defs tab with an id selected.
fn cross_link(state: AppState, tab: DefsTab, id: String, label: String) -> Element {
    let mut state = state;
    rsx! {
        button {
            class: "link",
            onclick: move |_| {
                let mut ds = state.defs_state.write();
                ds.tab = tab;
                ds.selected = Some(id.clone());
                ds.filter = String::new();
            },
            "{label}"
        }
    }
}

#[component]
fn EventsTab() -> Element {
    let state = AppState::use_ctx();
    let Some(defs) = current_defs(&state) else {
        return rsx! {};
    };
    let ds = state.defs_state.read().clone();
    let ids: Vec<(String, String)> = defs
        .events
        .iter()
        .map(|(k, e)| (k.clone(), e.description.clone()))
        .collect();

    let detail = match ds
        .selected
        .as_ref()
        .and_then(|id| defs.events.get(id).map(|e| (id, e)))
    {
        Some((id, event)) => rsx! {
            div { class: "narr-card",
                div { class: "narr-head",
                    div { class: "narr-eyebrow", "Trigger event" }
                    div { class: "narr-title", "{id}" }
                    div { class: "narr-sub",
                        if event.msg_struct_id != *id {
                            "structure {event.msg_struct_id} · "
                        }
                        "{event.description}"
                    }
                }
                div { class: "narr-body",
                    {structure_tree(state, &event.structure, 0)}
                }
            }
        },
        None => rsx! {
            div { class: "caption", "Select a trigger event to inspect its structure." }
        },
    };

    rsx! {
        MasterDetail { ids, detail }
    }
}

fn structure_tree(state: AppState, items: &[StructureItem], depth: usize) -> Element {
    rsx! {
        for item in items.iter() {
            match item {
                StructureItem::Group { group, usage, rpt, children } => rsx! {
                    div { class: "struct-row", style: "padding-left: {depth * 18}px;",
                        span { class: "struct-group", "{group}" }
                        {usage_chip(usage)}
                        {rpt_chip(rpt)}
                    }
                    {structure_tree(state, children, depth + 1)}
                },
                StructureItem::Segment { segment, usage, rpt } => rsx! {
                    div { class: "struct-row", style: "padding-left: {depth * 18}px;",
                        {cross_link(state, DefsTab::Segments, segment.clone(), segment.clone())}
                        {usage_chip(usage)}
                        {rpt_chip(rpt)}
                    }
                },
            }
        }
    }
}

#[component]
fn SegmentsTab() -> Element {
    let state = AppState::use_ctx();
    let Some(defs) = current_defs(&state) else {
        return rsx! {};
    };
    let ds = state.defs_state.read().clone();
    let ids: Vec<(String, String)> = defs
        .segments
        .iter()
        .map(|(k, s)| (k.clone(), s.name.clone()))
        .collect();

    let detail = match ds
        .selected
        .as_ref()
        .and_then(|id| defs.segments.get(id).map(|s| (id, s)))
    {
        Some((id, seg)) => rsx! {
            div { class: "narr-card",
                div { class: "narr-head",
                    div { class: "narr-eyebrow", "Segment" }
                    div { class: "narr-title", "{id} - {seg.name}" }
                    div { class: "narr-sub", "{seg.fields.len()} fields" }
                }
                div { class: "narr-body",
                    for (i, field) in seg.fields.iter().enumerate() {
                        div { class: "narr-row",
                            span { class: "narr-k", "{i + 1}. {field.name}" }
                            span { class: "narr-v",
                                {cross_link(state, DefsTab::DataTypes, field.data_type.clone(), field.data_type.clone())}
                                " "
                                {usage_chip(&field.usage)}
                                " "
                                {rpt_chip(&field.rpt)}
                                if field.length > 0 {
                                    span { class: "json-sep", " len {field.length}" }
                                }
                                if let Some(table) = &field.table {
                                    " "
                                    {cross_link(state, DefsTab::Tables, table.clone(), format!("table {table}"))}
                                }
                            }
                        }
                    }
                }
            }
        },
        None => rsx! {
            div { class: "caption", "Select a segment to inspect its fields." }
        },
    };

    rsx! {
        MasterDetail { ids, detail }
    }
}

#[component]
fn DataTypesTab() -> Element {
    let state = AppState::use_ctx();
    let Some(defs) = current_defs(&state) else {
        return rsx! {};
    };
    let ds = state.defs_state.read().clone();
    let ids: Vec<(String, String)> = defs
        .data_types
        .iter()
        .map(|(k, d)| (k.clone(), d.name.clone()))
        .collect();

    let detail = match ds
        .selected
        .as_ref()
        .and_then(|id| defs.data_types.get(id).map(|d| (id, d)))
    {
        Some((id, dt)) => rsx! {
            div { class: "narr-card",
                div { class: "narr-head",
                    div { class: "narr-eyebrow", "Data type" }
                    div { class: "narr-title", "{id} - {dt.name}" }
                    div { class: "narr-sub",
                        if dt.components.is_empty() {
                            "primitive type"
                        } else {
                            "{dt.components.len()} components"
                        }
                    }
                }
                div { class: "narr-body",
                    if dt.components.is_empty() {
                        div { class: "caption", "Primitive type - no components." }
                    }
                    for (i, comp) in dt.components.iter().enumerate() {
                        div { class: "narr-row",
                            span { class: "narr-k", "{i + 1}. {comp.name}" }
                            span { class: "narr-v",
                                {cross_link(state, DefsTab::DataTypes, comp.data_type.clone(), comp.data_type.clone())}
                                " "
                                {usage_chip(&comp.usage)}
                                if comp.length > 0 {
                                    span { class: "json-sep", " len {comp.length}" }
                                }
                                if let Some(table) = &comp.table {
                                    " "
                                    {cross_link(state, DefsTab::Tables, table.clone(), format!("table {table}"))}
                                }
                            }
                        }
                    }
                }
            }
        },
        None => rsx! {
            div { class: "caption", "Select a data type to inspect its components." }
        },
    };

    rsx! {
        MasterDetail { ids, detail }
    }
}

#[component]
fn TablesTab() -> Element {
    let state = AppState::use_ctx();
    let Some(defs) = current_defs(&state) else {
        return rsx! {};
    };
    let ds = state.defs_state.read().clone();
    let ids: Vec<(String, String)> = defs
        .tables
        .iter()
        .map(|(k, t)| (k.clone(), t.name.clone()))
        .collect();

    let detail = match ds
        .selected
        .as_ref()
        .and_then(|id| defs.tables.get(id).map(|t| (id, t)))
    {
        Some((id, table)) => rsx! {
            div { class: "narr-card",
                div { class: "narr-head",
                    div { class: "narr-eyebrow", "{table.table_type} table" }
                    div { class: "narr-title", "{id} - {table.name}" }
                    div { class: "narr-sub", "{table.values.len()} values" }
                }
                div { class: "narr-body",
                    for (value, description) in table.values.iter() {
                        div { class: "narr-row",
                            span { class: "narr-k", style: "font-family: var(--font-mono);", "{value}" }
                            span { class: "narr-v", style: "font-family: var(--font-body);", "{description}" }
                        }
                    }
                }
            }
        },
        None => rsx! {
            div { class: "caption", "Select a table to inspect its values." }
        },
    };

    rsx! {
        MasterDetail { ids, detail }
    }
}
