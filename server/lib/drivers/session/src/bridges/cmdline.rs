//! Command-line extension state bridge.
//!
//! Adapts [`CmdlineState`] to JSON for gRPC transmission to clients.

use crate::{CmdlineState, ExtensionMap};

use super::{ExtensionScope, ExtensionStateBridge};

/// Bridge for command-line state.
///
/// Reads `CmdlineState` from the client's `ExtensionMap` and serializes
/// it to JSON with fields: `active`, `prompt`, `input`, `cursor`.
pub struct CmdlineBridge;

impl ExtensionStateBridge for CmdlineBridge {
    fn kind(&self) -> &'static str {
        "cmdline"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<CmdlineState>()?;
        Some(serde_json::json!({
            "active": state.is_active(),
            "prompt": state.prompt().char().to_string(),
            "input": state.input(),
            "cursor": state.cursor(),
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<CmdlineState>()
            .is_some_and(CmdlineState::is_active)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::CmdlinePrompt;

    use super::*;

    #[test]
    fn test_cmdline_bridge_kind() {
        assert_eq!(CmdlineBridge.kind(), "cmdline");
    }

    #[test]
    fn test_cmdline_bridge_scope() {
        assert_eq!(CmdlineBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn test_cmdline_bridge_snapshot_empty() {
        let map = ExtensionMap::new();
        assert!(CmdlineBridge.snapshot(&map).is_none());
    }

    #[test]
    fn test_cmdline_bridge_snapshot_active() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::Command);
        state.insert_char('w');
        state.insert_char('q');

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert_eq!(snap["prompt"], ":");
        assert_eq!(snap["input"], "wq");
        assert_eq!(snap["cursor"], 2);
    }

    #[test]
    fn test_cmdline_bridge_snapshot_inactive() {
        let mut map = ExtensionMap::new();
        // get_or_insert creates default (inactive) state
        map.get_or_insert::<CmdlineState>();

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
        assert_eq!(snap["prompt"], ":");
        assert_eq!(snap["input"], "");
        assert_eq!(snap["cursor"], 0);
    }

    #[test]
    fn test_cmdline_bridge_snapshot_search_prompt() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::SearchForward);
        state.insert_char('f');
        state.insert_char('o');
        state.insert_char('o');

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        assert_eq!(snap["prompt"], "/");
        assert_eq!(snap["input"], "foo");
    }

    #[test]
    fn test_cmdline_bridge_is_active_empty() {
        let map = ExtensionMap::new();
        assert!(!CmdlineBridge.is_active(&map));
    }

    #[test]
    fn test_cmdline_bridge_is_active_true() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::Command);
        assert!(CmdlineBridge.is_active(&map));
    }

    #[test]
    fn test_cmdline_bridge_is_active_false_after_exit() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::Command);
        state.exit();
        assert!(!CmdlineBridge.is_active(&map));
    }
}
