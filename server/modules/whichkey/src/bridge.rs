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
        reovim_extension_kinds::WHICHKEY
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
#[path = "bridge_tests.rs"]
mod tests;
