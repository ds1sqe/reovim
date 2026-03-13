//! Extension state bridge for the settings panel.
//!
//! Serializes settings panel state to JSON for transmission to TUI/web clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::{
    KIND,
    state::{FlatItem, SettingKind, SettingsState},
};

/// Bridge that serializes settings panel state to clients.
pub struct SettingsBridge;

impl ExtensionStateBridge for SettingsBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<SettingsState>()?;

        if !state.open {
            return None;
        }

        let items: Vec<serde_json::Value> = state
            .items
            .iter()
            .map(|item| match item {
                FlatItem::SectionHeader { title } => {
                    serde_json::json!({
                        "type": "header",
                        "title": title,
                    })
                }
                FlatItem::Setting {
                    name,
                    description,
                    value,
                    kind,
                } => {
                    serde_json::json!({
                        "type": "setting",
                        "name": name,
                        "description": description,
                        "value": value,
                        "kind": kind_to_str(*kind),
                    })
                }
            })
            .collect();

        Some(serde_json::json!({
            "open": true,
            "items": items,
            "selected_index": state.selected_index,
            "scroll_offset": state.scroll_offset,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions.get::<SettingsState>().is_some_and(|s| s.open)
    }
}

/// Convert setting kind to string for serialization.
const fn kind_to_str(kind: SettingKind) -> &'static str {
    match kind {
        SettingKind::Bool => "bool",
        SettingKind::Int => "int",
        SettingKind::String => "string",
        SettingKind::Choice => "choice",
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
