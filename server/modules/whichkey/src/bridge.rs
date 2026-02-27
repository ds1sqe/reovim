//! Which-key extension state bridge.
//!
//! Adapts [`PendingBindings`] to JSON for gRPC transmission to clients.
//! Shows the accumulated pending key prefix and available continuations
//! so the client can display a which-key popup.

use {
    reovim_driver_input::PendingBindings,
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
};

/// Bridge for which-key popup state.
///
/// Reads [`PendingBindings`] from the client's `ExtensionMap` and serializes
/// it to JSON with fields: `prefix`, `hints`.
///
/// This bridge has **zero dependency on vim** -- it reads the generic
/// `PendingBindings` extension populated by the session layer after any
/// resolver returns `Pending`.
pub struct WhichKeyBridge;

impl ExtensionStateBridge for WhichKeyBridge {
    fn kind(&self) -> &'static str {
        "whichkey"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let pb = extensions.get::<PendingBindings>()?;
        if !pb.is_active() {
            // Return inactive state so the TUI clears the popup.
            // Without this, no ExtensionUpdated notification would be sent
            // on deactivation (build_extension_notification skips None snapshots).
            return Some(serde_json::json!({
                "active": false,
                "prefix": "",
                "hints": [],
            }));
        }
        let prefix = if pb.mode_prefix.is_empty() {
            format!("{}", pb.pending_keys)
        } else if pb.pending_keys.is_empty() {
            format!("{}", pb.mode_prefix)
        } else {
            format!("{}{}", pb.mode_prefix, pb.pending_keys)
        };
        Some(serde_json::json!({
            "active": true,
            "prefix": prefix,
            "hints": pb.continuations.iter().map(|(keys, cmd)| {
                serde_json::json!({
                    "key": format!("{keys}"),
                    "command": format!("{cmd}"),
                })
            }).collect::<Vec<_>>(),
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<PendingBindings>()
            .is_some_and(PendingBindings::is_active)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_input::KeySequence,
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    #[test]
    fn test_whichkey_bridge_kind() {
        assert_eq!(WhichKeyBridge.kind(), "whichkey");
    }

    #[test]
    fn test_whichkey_bridge_scope() {
        assert_eq!(WhichKeyBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn test_whichkey_bridge_snapshot_empty() {
        let map = ExtensionMap::new();
        assert!(WhichKeyBridge.snapshot(&map).is_none());
    }

    #[test]
    fn test_whichkey_bridge_snapshot_inactive() {
        let mut map = ExtensionMap::new();
        // Create PendingBindings but leave it inactive (no continuations)
        map.get_or_insert::<PendingBindings>();

        // Returns Some with active:false so TUI gets deactivation notification
        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
        assert_eq!(snap["prefix"], "");
        assert!(snap["hints"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_whichkey_bridge_snapshot_with_hints() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();

        // Simulate pending "g" with two continuations
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('g'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('g'),
            )]),
            CommandId::new(TEST_MODULE, "goto-top"),
        ));
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('d'),
            )]),
            CommandId::new(TEST_MODULE, "goto-definition"),
        ));

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert!(snap["prefix"].as_str().unwrap().contains('g'));

        let hints = snap["hints"].as_array().unwrap();
        assert_eq!(hints.len(), 2);
        assert!(hints[0]["command"].as_str().unwrap().contains("goto-top"));
        assert!(
            hints[1]["command"]
                .as_str()
                .unwrap()
                .contains("goto-definition")
        );
    }

    #[test]
    fn test_whichkey_bridge_snapshot_mode_prefix_only() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();

        // Simulate operator push: "d" sets mode_prefix, pending_keys empty
        pb.mode_prefix = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('d'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('w'),
            )]),
            CommandId::new(TEST_MODULE, "delete-word"),
        ));

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert_eq!(snap["prefix"].as_str().unwrap(), "d");
    }

    #[test]
    fn test_whichkey_bridge_snapshot_mode_prefix_with_pending() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();

        // Simulate "di": mode_prefix="d", pending_keys="i"
        pb.mode_prefix = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('d'),
        )]);
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('i'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('w'),
            )]),
            CommandId::new(TEST_MODULE, "inner-word"),
        ));

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert_eq!(snap["prefix"].as_str().unwrap(), "di");
    }

    #[test]
    fn test_whichkey_bridge_is_active_empty() {
        let map = ExtensionMap::new();
        assert!(!WhichKeyBridge.is_active(&map));
    }

    #[test]
    fn test_whichkey_bridge_is_active_no_pending() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<PendingBindings>();
        assert!(!WhichKeyBridge.is_active(&map));
    }

    #[test]
    fn test_whichkey_bridge_is_active_with_pending() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.continuations
            .push((KeySequence::new(), CommandId::new(TEST_MODULE, "test-cmd")));
        assert!(WhichKeyBridge.is_active(&map));
    }

    #[test]
    fn test_whichkey_bridge_is_active_after_clear() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.continuations
            .push((KeySequence::new(), CommandId::new(TEST_MODULE, "test-cmd")));
        assert!(WhichKeyBridge.is_active(&map));

        let pb = map.get_or_insert::<PendingBindings>();
        pb.clear();
        assert!(!WhichKeyBridge.is_active(&map));
    }
}
