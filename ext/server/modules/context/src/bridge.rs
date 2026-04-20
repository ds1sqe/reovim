//! Extension state bridge for scope context.
//!
//! Serializes context hierarchy to JSON for transmission to TUI/web clients.

use reovim_driver_text_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::state::ContextSessionState;

const KIND: &str = "context";

/// Bridge that serializes scope context state to clients.
pub struct ContextBridge;

impl ExtensionStateBridge for ContextBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        // Per-client: each client has its own cursor -> different scope context
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<ContextSessionState>()?;

        let hierarchy = state.hierarchy();
        let active = hierarchy.is_some_and(|h| !h.is_empty());

        let breadcrumb = state.breadcrumb_text();

        let items: Vec<serde_json::Value> = hierarchy
            .map(|h| {
                h.items
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "start_line": s.start_line,
                            "end_line": s.end_line,
                            "kind": s.kind.as_str(),
                            "display_text": s.display_text,
                            "name": s.name,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(serde_json::json!({
            "active": active,
            "breadcrumb": breadcrumb,
            "items": items,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<ContextSessionState>()
            .is_some_and(|state| state.hierarchy().is_some_and(|h| !h.is_empty()))
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
