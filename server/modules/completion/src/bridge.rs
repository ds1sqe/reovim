//! Completion extension state bridge.
//!
//! Serializes [`CompletionState`] to JSON for gRPC transmission to clients.
//! Both TUI (Rust) and Web (TypeScript) extensions consume the same JSON.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::{KIND, state::CompletionState};

/// Bridge for completion popup state.
///
/// Reads [`CompletionState`] from the client's `ExtensionMap` and serializes
/// it to JSON with fields matching what TUI and Web extensions expect.
pub struct CompletionBridge;

impl ExtensionStateBridge for CompletionBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<CompletionState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        let items: Vec<serde_json::Value> = state
            .items
            .iter()
            .map(|item| {
                let mut obj = serde_json::json!({
                    "label": item.label,
                    "kindAbbrev": item.kind_abbrev,
                    "sourceId": item.source_id,
                });
                if let Some(ref detail) = item.detail {
                    obj["detail"] = serde_json::json!(detail);
                }
                obj
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "items": items,
            "selected": state.selected,
            "prefix": state.prefix,
            "scrollOffset": state.scroll_offset,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<CompletionState>()
            .is_some_and(|s| s.active)
    }

    fn on_mode_changed(&self, _from: &str, _to: &str, extensions: &mut ExtensionMap) {
        // Auto-dismiss completion popup on any mode change (#521).
        // Mode changed = user is doing something else. No string coupling needed.
        if let Some(state) = extensions.get_mut::<CompletionState>()
            && state.active
        {
            state.close();
        }
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
