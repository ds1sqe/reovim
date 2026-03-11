//! Signature help extension state bridge.
//!
//! Serializes [`SignatureHelpState`] to JSON for gRPC transmission to clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::signature_help_state::SignatureHelpState;

/// Bridge for signature help popup state.
///
/// Reads [`SignatureHelpState`] from the client's `ExtensionMap` and
/// serializes it to JSON with `SemanticOrigin`-style origin fields.
pub struct SignatureHelpBridge;

impl ExtensionStateBridge for SignatureHelpBridge {
    fn kind(&self) -> &'static str {
        reovim_extension_kinds::SIGNATURE_HELP
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<SignatureHelpState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        Some(serde_json::json!({
            "active": true,
            "label": state.label,
            "origin": {
                "BufferPosition": {
                    "buffer_id": state.origin_buffer_id,
                    "line": state.origin_line,
                    "col": state.origin_col,
                }
            }
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<SignatureHelpState>()
            .is_some_and(|s| s.active)
    }

    /// Auto-dismiss signature help on any mode change.
    /// Mode changed = user is doing something else, dismiss the popup.
    fn on_mode_changed(&self, _from: &str, _to: &str, extensions: &mut ExtensionMap) {
        if let Some(state) = extensions.get_mut::<SignatureHelpState>()
            && state.active
        {
            state.dismiss();
        }
    }
}

#[cfg(test)]
#[path = "signature_help_bridge_tests.rs"]
mod tests;
