//! Hover extension state bridge.
//!
//! Serializes [`HoverState`] to JSON for gRPC transmission to clients.
//! Both TUI (Rust) and Web (TypeScript) extensions consume the same JSON.

use {
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::ServiceRegistry,
};

use crate::{
    KIND_HOVER,
    hover_state::{HoverCache, HoverContentType, HoverState},
};

/// Bridge for hover popup state.
///
/// Reads [`HoverState`] from the client's `ExtensionMap` and serializes
/// it to JSON with `SemanticOrigin`-style origin fields.
pub struct HoverBridge;

impl ExtensionStateBridge for HoverBridge {
    fn kind(&self) -> &'static str {
        KIND_HOVER
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<HoverState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        let content_type = match state.content_type {
            HoverContentType::PlainText => "plaintext",
            HoverContentType::Markdown => "markdown",
        };

        Some(serde_json::json!({
            "active": true,
            "content": state.content,
            "contentType": content_type,
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
        extensions.get::<HoverState>().is_some_and(|s| s.active)
    }

    fn on_mode_changed(&self, _from: &str, _to: &str, extensions: &mut ExtensionMap) {
        // Auto-dismiss hover on any mode change.
        // Mode changed = user is doing something else.
        if let Some(state) = extensions.get_mut::<HoverState>()
            && state.active
        {
            state.dismiss();
        }
    }

    fn tick(
        &self,
        client_extensions: &mut ExtensionMap,
        _shared_extensions: &mut ExtensionMap,
        services: &ServiceRegistry,
    ) -> bool {
        // Check HoverCache for a pending async hover result.
        let Some(cache) = services.get::<HoverCache>() else {
            return false;
        };
        let Some(snapshot) = cache.take() else {
            return false;
        };

        // Move the snapshot into per-client HoverState.
        let state = client_extensions.get_or_insert::<HoverState>();
        state.show(
            snapshot.content,
            snapshot.content_type,
            snapshot.buffer_id,
            snapshot.line,
            snapshot.col,
        );
        true
    }
}

#[cfg(test)]
#[path = "hover_bridge_tests.rs"]
mod tests;
