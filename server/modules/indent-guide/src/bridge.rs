//! Extension state bridge for indent guide rendering.
//!
//! Serializes indent guide state to JSON for transmission to TUI/web clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::{KIND, state::IndentGuideState};

/// Bridge that serializes indent guide state to clients.
pub struct IndentGuideBridge;

impl ExtensionStateBridge for IndentGuideBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<IndentGuideState>()?;

        if !state.options.enabled {
            return None;
        }

        let guides: Vec<serde_json::Value> = state
            .guides
            .iter()
            .map(|g| {
                serde_json::json!({
                    "line": g.line,
                    "level": g.level,
                    "active": g.active,
                })
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "guide_char": state.options.guide_char.to_string(),
            "tab_size": state.options.tab_size,
            "guides": guides,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<IndentGuideState>()
            .is_some_and(|s| s.options.enabled)
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
