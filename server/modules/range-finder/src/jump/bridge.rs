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
mod tests {
    use {super::*, crate::jump::search::Direction, reovim_driver_session::TextInputSink};

    #[test]
    fn test_jump_bridge_kind() {
        assert_eq!(JumpBridge.kind(), "range-finder-jump");
    }

    #[test]
    fn test_jump_bridge_scope() {
        assert_eq!(JumpBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn test_jump_bridge_snapshot_empty_map() {
        let map = ExtensionMap::new();
        assert!(JumpBridge.snapshot(&map).is_none());
    }

    #[test]
    fn test_jump_bridge_snapshot_inactive() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<JumpSessionState>();

        let snap = JumpBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
    }

    #[test]
    fn test_jump_bridge_snapshot_active() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<JumpSessionState>();
        state.start(vec!["he he he".into()], 0, 100, Direction::Both);
        state.insert_char('h');
        state.insert_char('e');

        let snap = JumpBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        let matches = snap["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0]["label"], "s");
    }

    #[test]
    fn test_jump_bridge_is_active_empty() {
        let map = ExtensionMap::new();
        assert!(!JumpBridge.is_active(&map));
    }

    #[test]
    fn test_jump_bridge_is_active_true() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<JumpSessionState>();
        state.start(vec!["hello".into()], 0, 0, Direction::Both);
        assert!(JumpBridge.is_active(&map));
    }

    #[test]
    fn test_jump_bridge_is_active_false() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<JumpSessionState>();
        assert!(!JumpBridge.is_active(&map));
    }

    #[test]
    fn test_jump_bridge_snapshot_active_waiting_first() {
        // Active but not yet showing labels (WaitingFirstChar).
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<JumpSessionState>();
        state.start(vec!["hello".into()], 0, 0, Direction::Both);

        let snap = JumpBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        // No matches yet, empty array.
        let matches = snap["matches"].as_array().unwrap();
        assert!(matches.is_empty());
    }
}
