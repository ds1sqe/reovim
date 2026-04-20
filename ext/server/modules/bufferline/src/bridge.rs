//! Bufferline pin state bridge.
//!
//! Emits only the pin list to clients. Buffer list data is fetched
//! client-side via `list_buffers()` gRPC — this bridge handles only
//! the server-owned pin state.

use {
    reovim_driver_text_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::ServiceRegistry,
};

use crate::{KIND, state::BufferlineState};

/// Bridge for bufferline pin state.
///
/// Emits `{"type":"pin_state","pins":[...]}` when pin list changes.
pub struct PinBridge;

impl ExtensionStateBridge for PinBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Shared
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<BufferlineState>()?;
        Some(serde_json::json!({
            "type": "pin_state",
            "pins": state.pinned,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions.get::<BufferlineState>().is_some()
    }

    fn tick(
        &self,
        _client_extensions: &mut ExtensionMap,
        _shared_extensions: &mut ExtensionMap,
        _services: &ServiceRegistry,
    ) -> bool {
        // Pin state changes are detected by detect_bridge_changes() in
        // input.rs which calls is_active() + snapshot() on every keypress.
        // No periodic tick needed.
        false
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
