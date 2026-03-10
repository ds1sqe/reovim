//! Command-line extension state bridge.
//!
//! Adapts [`CmdlineState`] to JSON for gRPC transmission to clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::CmdlineState;

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
        let mut json = serde_json::json!({
            "active": state.is_active(),
            "prompt": state.prompt().char().to_string(),
            "input": state.input(),
            "cursor": state.cursor(),
            "completions": state.completions(),
            "completion_index": state.completion_index(),
        });
        if let Some(msg) = state.message() {
            json["message"] = serde_json::Value::String(msg.text().to_owned());
            json["message_kind"] = serde_json::Value::String(msg.kind().to_owned());
        }
        Some(json)
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<CmdlineState>()
            .is_some_and(|state| state.is_active() || state.message().is_some())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{CmdlineMessage, CmdlinePrompt},
    };

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
    fn test_cmdline_bridge_snapshot_with_completions() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::Command);
        state.set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);
        state.complete_next();

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        let completions = snap["completions"].as_array().unwrap();
        assert_eq!(completions.len(), 2);
        assert_eq!(completions[0], "write");
        assert_eq!(completions[1], "wq");
        assert_eq!(snap["completion_index"], 0);
    }

    #[test]
    fn test_cmdline_bridge_snapshot_no_completions() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::Command);

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        let completions = snap["completions"].as_array().unwrap();
        assert!(completions.is_empty());
        assert!(snap["completion_index"].is_null());
    }

    #[test]
    fn test_cmdline_bridge_is_active_false_after_exit() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::Command);
        state.exit();
        assert!(!CmdlineBridge.is_active(&map));
    }

    // -- Message bridge tests (#558) --

    #[test]
    fn test_cmdline_bridge_snapshot_with_error_message() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.set_message(CmdlineMessage::Error("E492: Not an editor command".to_string()));

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        assert_eq!(snap["message"], "E492: Not an editor command");
        assert_eq!(snap["message_kind"], "error");
    }

    #[test]
    fn test_cmdline_bridge_snapshot_with_info_message() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.set_message(CmdlineMessage::Info("3 lines yanked".to_string()));

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        assert_eq!(snap["message"], "3 lines yanked");
        assert_eq!(snap["message_kind"], "info");
    }

    #[test]
    fn test_cmdline_bridge_snapshot_no_message() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<CmdlineState>();

        let snap = CmdlineBridge.snapshot(&map).unwrap();
        assert!(snap.get("message").is_none());
        assert!(snap.get("message_kind").is_none());
    }

    #[test]
    fn test_cmdline_bridge_is_active_with_message_only() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        // Not active, but has a message
        assert!(!state.is_active());
        state.set_message(CmdlineMessage::Error("test".to_string()));
        assert!(CmdlineBridge.is_active(&map));
    }

    #[test]
    fn test_cmdline_bridge_is_active_after_exit_with_message() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CmdlineState>();
        state.enter(CmdlinePrompt::Command);
        state.exit();
        // After exit, not active...
        assert!(!CmdlineBridge.is_active(&map));
        // ...but with a message, it should be active
        let state = map.get_or_insert::<CmdlineState>();
        state.set_message(CmdlineMessage::Error("E492: foo".to_string()));
        assert!(CmdlineBridge.is_active(&map));
    }
}
