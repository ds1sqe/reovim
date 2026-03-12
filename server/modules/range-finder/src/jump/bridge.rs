//! Jump extension state bridge.
//!
//! Adapts [`JumpSessionState`] to JSON for gRPC transmission to clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use super::state::JumpSessionState;

/// Bridge for jump navigation state.
///
/// Reads `JumpSessionState` from the client's `ExtensionMap` and serializes
/// it to JSON with fields: `active`, `matches` (array of label overlays).
pub struct JumpBridge;

impl ExtensionStateBridge for JumpBridge {
    fn kind(&self) -> &'static str {
        "range-finder-jump"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<JumpSessionState>()?;
        if !state.is_active() {
            return Some(serde_json::json!({ "active": false }));
        }

        let matches = state.get_matches().map_or_else(Vec::new, |ms| {
            ms.iter()
                .map(|m| {
                    serde_json::json!({
                        "line": m.line,
                        "col": m.col,
                        "label": m.label,
                    })
                })
                .collect()
        });

        Some(serde_json::json!({
            "active": true,
            "matches": matches,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<JumpSessionState>()
            .is_some_and(JumpSessionState::is_active)
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
