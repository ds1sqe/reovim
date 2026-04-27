//! Extension state bridge for pair highlighting.
//!
//! Serializes bracket state to JSON for transmission to TUI/web clients.

use reovim_driver_text_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::{KIND, state::PairState};

/// Bridge that serializes pair state to clients.
pub struct PairBridge;

impl ExtensionStateBridge for PairBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        // Per-client: each client has its own cursor -> different matched pair
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<PairState>()?;

        let brackets: Vec<serde_json::Value> = state
            .brackets
            .iter()
            .map(|((_line, _col), info)| {
                serde_json::json!({
                    "line": info.line,
                    "col": info.col,
                    "depth": if info.depth == usize::MAX { u64::MAX } else { info.depth as u64 },
                    "char": info.ch.to_string(),
                    "unmatched": info.depth == usize::MAX,
                })
            })
            .collect();

        let matched = state.matched.map(|m| {
            serde_json::json!({
                "open": { "line": m.open.line, "col": m.open.col },
                "close": { "line": m.close.line, "col": m.close.col },
            })
        });

        Some(serde_json::json!({
            "active": true,
            "rainbow": state.options.rainbow,
            "matchpair": state.options.matchpair,
            "brackets": brackets,
            "matched": matched,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions.get::<PairState>().is_some()
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
