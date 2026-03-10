//! Hover extension state bridge.
//!
//! Serializes [`HoverState`] to JSON for gRPC transmission to clients.
//! Both TUI (Rust) and Web (TypeScript) extensions consume the same JSON.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::hover_state::{HoverContentType, HoverState};

/// Bridge for hover popup state.
///
/// Reads [`HoverState`] from the client's `ExtensionMap` and serializes
/// it to JSON with `SemanticOrigin`-style origin fields.
pub struct HoverBridge;

impl ExtensionStateBridge for HoverBridge {
    fn kind(&self) -> &'static str {
        "hover"
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_kind() {
        assert_eq!(HoverBridge.kind(), "hover");
    }

    #[test]
    fn bridge_scope() {
        assert_eq!(HoverBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn snapshot_no_state_returns_none() {
        let map = ExtensionMap::new();
        assert!(HoverBridge.snapshot(&map).is_none());
    }

    #[test]
    fn snapshot_inactive() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<HoverState>();

        let snap = HoverBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
        assert!(snap.get("content").is_none());
    }

    #[test]
    fn snapshot_active_plaintext() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<HoverState>();
        state.show("fn foo() -> bool".to_owned(), HoverContentType::PlainText, 1, 5, 12);

        let snap = HoverBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert_eq!(snap["content"], "fn foo() -> bool");
        assert_eq!(snap["contentType"], "plaintext");
        assert_eq!(snap["origin"]["BufferPosition"]["buffer_id"], 1);
        assert_eq!(snap["origin"]["BufferPosition"]["line"], 5);
        assert_eq!(snap["origin"]["BufferPosition"]["col"], 12);
    }

    #[test]
    fn snapshot_active_markdown() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<HoverState>();
        state.show("**bold**".to_owned(), HoverContentType::Markdown, 2, 10, 0);

        let snap = HoverBridge.snapshot(&map).unwrap();
        assert_eq!(snap["contentType"], "markdown");
        assert_eq!(snap["content"], "**bold**");
        assert_eq!(snap["origin"]["BufferPosition"]["buffer_id"], 2);
    }

    #[test]
    fn is_active_no_state() {
        let map = ExtensionMap::new();
        assert!(!HoverBridge.is_active(&map));
    }

    #[test]
    fn is_active_inactive_state() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<HoverState>();
        assert!(!HoverBridge.is_active(&map));
    }

    #[test]
    fn is_active_active_state() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<HoverState>();
        state.active = true;
        assert!(HoverBridge.is_active(&map));
    }

    #[test]
    fn on_mode_changed_dismisses_active() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<HoverState>();
        state.show("hover".to_owned(), HoverContentType::PlainText, 1, 0, 0);
        assert!(state.active);

        HoverBridge.on_mode_changed("test:normal", "test:insert", &mut map);

        let state = map.get::<HoverState>().unwrap();
        assert!(!state.active);
        assert!(state.content.is_empty());
    }

    #[test]
    fn on_mode_changed_noop_when_inactive() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<HoverState>();

        HoverBridge.on_mode_changed("test:normal", "test:insert", &mut map);

        let state = map.get::<HoverState>().unwrap();
        assert!(!state.active);
    }

    #[test]
    fn on_mode_changed_noop_no_state() {
        let mut map = ExtensionMap::new();
        HoverBridge.on_mode_changed("test:normal", "test:insert", &mut map);
    }
}
