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
#[path = "bridge_tests.rs"]
mod tests;
