//! Extension state bridge for sticky context headers.
//!
//! Serializes header rows to JSON for TUI/web clients.

use {
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_module_context::ContextSessionState,
};

use crate::state::StickyContextState;

const KIND: &str = "sticky-context";

/// Bridge that serializes sticky context header rows to clients.
pub struct StickyContextBridge;

impl ExtensionStateBridge for StickyContextBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let sticky = extensions.get::<StickyContextState>()?;
        let context = extensions.get::<ContextSessionState>();

        // Use the context hierarchy's line as an approximation of viewport top.
        // In practice, the runner would set viewport_top from the client's
        // scroll position. For now, use the cursor line from hierarchy metadata.
        let viewport_top = context.and_then(|c| c.hierarchy()).map_or(0, |h| h.line);

        let rows = sticky.header_rows(context, viewport_top);

        if rows.is_empty() {
            return None;
        }

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "line": row.line,
                    "text": row.text,
                    "kind": row.kind,
                })
            })
            .collect();

        Some(serde_json::json!({
            "headers": items,
            "separator": sticky.options.separator,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<StickyContextState>()
            .is_some_and(|s| s.options.enabled)
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
