//! Fold extension state bridge.
//!
//! Adapts [`FoldSessionState`] to JSON for gRPC transmission to clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use super::state::FoldSessionState;
use crate::KIND_FOLD;

/// Bridge for fold state.
///
/// Reads `FoldSessionState` from the session's `ExtensionMap` and serializes
/// collapsed folds to JSON. Scope is `Shared` since folds are per-buffer
/// (shared across all clients in a session).
pub struct FoldBridge;

impl ExtensionStateBridge for FoldBridge {
    fn kind(&self) -> &'static str {
        KIND_FOLD
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Shared
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<FoldSessionState>()?;
        if !state.has_collapsed_folds() {
            return None;
        }

        let mut folds = serde_json::Map::new();

        for (buf_id, fold) in state.buffers() {
            if !fold.has_collapsed() {
                continue;
            }
            let collapsed: Vec<serde_json::Value> = fold
                .collapsed_info()
                .map(|(start_line, hidden_count, preview)| {
                    serde_json::json!({
                        "start_line": start_line,
                        "hidden_count": hidden_count,
                        "preview": preview,
                    })
                })
                .collect();

            folds.insert(buf_id.to_string(), serde_json::Value::Array(collapsed));
        }

        Some(serde_json::json!({ "folds": folds }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<FoldSessionState>()
            .is_some_and(FoldSessionState::has_collapsed_folds)
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
