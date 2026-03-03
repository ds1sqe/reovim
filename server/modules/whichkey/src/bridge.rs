//! Which-key extension state bridge.
//!
//! Adapts [`PendingBindings`] to JSON for gRPC transmission to clients.
//! Shows the accumulated pending key prefix and available continuations
//! so the client can display a which-key popup.
//!
//! # Filtering (#459)
//!
//! When a [`WhichKeyFilterConfig`] extension is present in the
//! `ExtensionMap`, the bridge applies its filters before emitting hints.
//! This allows filtering by category, key pattern, and binding layer.

use {
    reovim_driver_input::PendingBindings,
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
};

use crate::filter::WhichKeyFilterConfig;

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

        // Read optional filter config
        let filter = extensions.get::<WhichKeyFilterConfig>();

        let hints: Vec<_> = pb
            .continuations
            .iter()
            .filter(|(keys, info)| filter.is_none_or(|f| f.matches(keys, info)))
            .map(|(keys, info)| {
                serde_json::json!({
                    "key": format!("{keys}"),
                    "command": format!("{}", info.command),
                    "description": info.description,
                    "category": info.category.unwrap_or(""),
                    "layer": info.layer.name(),
                })
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "prefix": prefix,
            "hints": hints,
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
        crate::filter::BindingLayerFilter,
        reovim_driver_input::{BindingInfo, BindingLayer, KeySequence},
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    fn info(name: &'static str, desc: &'static str, cat: Option<&'static str>) -> BindingInfo {
        BindingInfo::new(CommandId::new(TEST_MODULE, name), desc, cat, BindingLayer::Policy)
    }

    fn info_at_layer(
        name: &'static str,
        desc: &'static str,
        cat: Option<&'static str>,
        layer: BindingLayer,
    ) -> BindingInfo {
        BindingInfo::new(CommandId::new(TEST_MODULE, name), desc, cat, layer)
    }

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
            info("goto-top", "Go to first line", Some("motion")),
        ));
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('d'),
            )]),
            info("goto-definition", "Go to definition", Some("motion")),
        ));

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert!(snap["prefix"].as_str().unwrap().contains('g'));

        let hints = snap["hints"].as_array().unwrap();
        assert_eq!(hints.len(), 2);
        assert!(hints[0]["command"].as_str().unwrap().contains("goto-top"));
        assert_eq!(hints[0]["description"], "Go to first line");
        assert_eq!(hints[0]["category"], "motion");
        assert_eq!(hints[0]["layer"], "policy");
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
            info("delete-word", "Delete word", Some("operator")),
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
            info("inner-word", "Inner word", Some("textobject")),
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
            .push((KeySequence::new(), info("test-cmd", "", None)));
        assert!(WhichKeyBridge.is_active(&map));
    }

    #[test]
    fn test_whichkey_bridge_is_active_after_clear() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.continuations
            .push((KeySequence::new(), info("test-cmd", "", None)));
        assert!(WhichKeyBridge.is_active(&map));

        let pb = map.get_or_insert::<PendingBindings>();
        pb.clear();
        assert!(!WhichKeyBridge.is_active(&map));
    }

    // ========================================================================
    // Filter tests (#459)
    // ========================================================================

    #[test]
    fn test_whichkey_bridge_snapshot_no_filter_passes_all() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('g'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('g'),
            )]),
            info("goto-top", "Top", Some("motion")),
        ));
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('d'),
            )]),
            info("goto-def", "Def", Some("navigation")),
        ));

        // No WhichKeyFilterConfig in extensions → all hints pass
        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        let hints = snap["hints"].as_array().unwrap();
        assert_eq!(hints.len(), 2);
    }

    #[test]
    fn test_whichkey_bridge_snapshot_filter_by_category() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('g'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('g'),
            )]),
            info("goto-top", "Top", Some("motion")),
        ));
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('d'),
            )]),
            info("goto-def", "Def", Some("navigation")),
        ));

        // Set category filter to "motion"
        let filter = map.get_or_insert::<WhichKeyFilterConfig>();
        filter.categories = Some(vec!["motion".to_owned()]);

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        let hints = snap["hints"].as_array().unwrap();
        assert_eq!(hints.len(), 1);
        assert!(hints[0]["command"].as_str().unwrap().contains("goto-top"));
    }

    #[test]
    fn test_whichkey_bridge_snapshot_filter_by_layer() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('g'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('g'),
            )]),
            info_at_layer("goto-top", "Top", Some("motion"), BindingLayer::Policy),
        ));
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('d'),
            )]),
            info_at_layer("custom-goto", "Custom", None, BindingLayer::User),
        ));

        // Filter to user-only
        let filter = map.get_or_insert::<WhichKeyFilterConfig>();
        filter.layer_filter = Some(BindingLayerFilter::UserOnly);

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        let hints = snap["hints"].as_array().unwrap();
        assert_eq!(hints.len(), 1);
        assert!(
            hints[0]["command"]
                .as_str()
                .unwrap()
                .contains("custom-goto")
        );
    }

    #[test]
    fn test_whichkey_bridge_snapshot_filter_defaults_only() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('g'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('g'),
            )]),
            info_at_layer("goto-top", "Top", None, BindingLayer::Policy),
        ));
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('d'),
            )]),
            info_at_layer("custom", "Custom", None, BindingLayer::User),
        ));

        // Filter to defaults only (Policy + Base, not User)
        let filter = map.get_or_insert::<WhichKeyFilterConfig>();
        filter.layer_filter = Some(BindingLayerFilter::DefaultsOnly);

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        let hints = snap["hints"].as_array().unwrap();
        assert_eq!(hints.len(), 1);
        assert!(hints[0]["command"].as_str().unwrap().contains("goto-top"));
    }

    #[test]
    fn test_whichkey_bridge_snapshot_no_category_field() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('g'),
        )]);
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('g'),
            )]),
            info("goto-top", "Top", None),
        ));

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        let hints = snap["hints"].as_array().unwrap();
        // No category → empty string in JSON
        assert_eq!(hints[0]["category"], "");
    }

    #[test]
    fn test_whichkey_bridge_snapshot_combined_filter() {
        let mut map = ExtensionMap::new();
        let pb = map.get_or_insert::<PendingBindings>();
        pb.pending_keys = KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
            reovim_driver_input::KeyCode::Char('g'),
        )]);
        // motion at Policy
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('g'),
            )]),
            info_at_layer("goto-top", "Top", Some("motion"), BindingLayer::Policy),
        ));
        // motion at User
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('j'),
            )]),
            info_at_layer("user-motion", "User motion", Some("motion"), BindingLayer::User),
        ));
        // operator at Policy
        pb.continuations.push((
            KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
                reovim_driver_input::KeyCode::Char('d'),
            )]),
            info_at_layer("delete", "Delete", Some("operator"), BindingLayer::Policy),
        ));

        // Filter: motion + defaults only → only "goto-top"
        let filter = map.get_or_insert::<WhichKeyFilterConfig>();
        filter.categories = Some(vec!["motion".to_owned()]);
        filter.layer_filter = Some(BindingLayerFilter::DefaultsOnly);

        let snap = WhichKeyBridge.snapshot(&map).unwrap();
        let hints = snap["hints"].as_array().unwrap();
        assert_eq!(hints.len(), 1);
        assert!(hints[0]["command"].as_str().unwrap().contains("goto-top"));
    }
}
