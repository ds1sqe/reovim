//! Hover extension state bridge.
//!
//! Serializes [`HoverState`] to JSON for gRPC transmission to clients.
//! Both TUI (Rust) and Web (TypeScript) extensions consume the same JSON.

use {
    reovim_driver_text_session::{
        CursorSnapshot, ExtensionMap, TextCursorShadow,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::ServiceRegistry,
    tracing::debug,
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

    #[allow(clippy::cast_possible_truncation)]
    fn on_cursor_moved(&self, cursor: &CursorSnapshot, extensions: &mut ExtensionMap) {
        // Dismiss hover when cursor moves away from the trigger position.
        if let Some(state) = extensions.get_mut::<HoverState>()
            && state.active
            && state.origin_cursor != *cursor.as_bytes()
        {
            state.dismiss();
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn tick(
        &self,
        client_extensions: &mut ExtensionMap,
        _shared_extensions: &mut ExtensionMap,
        services: &ServiceRegistry,
    ) -> bool {
        // Check HoverCache for a pending async hover result.
        let Some(cache) = services.get::<HoverCache>() else {
            debug!("hover tick: no HoverCache in services");
            return false;
        };
        let Some(snapshot) = cache.take() else {
            return false;
        };

        // Guard: suppress delivery if cursor has moved away from the
        // hover origin since the request was sent (#662). Without this,
        // a slow LSP response could show hover at a stale position.
        if let Some(cursor) = client_extensions
            .get::<TextCursorShadow>()
            .filter(|shadow| shadow.valid)
            && (cursor.buffer_id.as_usize() as u64 != snapshot.buffer_id
                || cursor.line != snapshot.line
                || cursor.col != snapshot.col)
        {
            debug!("hover tick: cursor moved, discarding stale snapshot");
            return false;
        }

        debug!(
            content_len = snapshot.content.len(),
            buffer_id = snapshot.buffer_id,
            line = snapshot.line,
            col = snapshot.col,
            "hover tick: delivering snapshot to HoverState"
        );

        let origin_cursor = client_extensions
            .get::<TextCursorShadow>()
            .filter(|shadow| shadow.valid)
            .map(TextCursorShadow::cursor_snapshot)
            .unwrap_or(CursorSnapshot::SENTINEL);

        // Move the snapshot into per-client HoverState.
        let state = client_extensions.get_or_insert::<HoverState>();
        state.show(
            snapshot.content,
            snapshot.content_type,
            snapshot.buffer_id,
            snapshot.line,
            snapshot.col,
        );
        state.set_origin_cursor(&origin_cursor);
        true
    }
}

#[cfg(test)]
#[path = "hover_bridge_tests.rs"]
mod tests;
