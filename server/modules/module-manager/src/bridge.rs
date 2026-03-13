//! Module manager extension state bridge (#622).
//!
//! Serializes [`ModuleManagerState`] to JSON for gRPC transmission to clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::state::ModuleManagerState;

/// Bridge for module manager state.
pub struct ModuleManagerBridge;

impl ExtensionStateBridge for ModuleManagerBridge {
    fn kind(&self) -> &'static str {
        reovim_extension_kinds::MODULE_MANAGER
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<ModuleManagerState>()
            .is_some_and(|s| s.active)
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<ModuleManagerState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        let filtered = state.filtered();
        let items: Vec<serde_json::Value> = filtered
            .iter()
            .map(|entry| {
                let mut obj = serde_json::json!({
                    "id": entry.id,
                    "version": entry.version,
                    "status": entry.status.label(),
                    "indicator": entry.status.indicator(),
                });
                if let Some(ref reason) = entry.reason {
                    obj["reason"] = serde_json::json!(reason);
                }
                obj
            })
            .collect();

        let detail = state.selected_entry().map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "version": entry.version,
                "status": entry.status.label(),
                "reason": entry.reason,
            })
        });

        Some(serde_json::json!({
            "active": true,
            "items": items,
            "selected": state.selected,
            "filter": state.filter.label(),
            "detailVisible": state.detail_visible,
            "detail": detail,
            "total": state.modules.len(),
            "filtered": filtered.len(),
        }))
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
