//! Notification extension state bridge.
//!
//! Adapts [`NotificationState`] to JSON for gRPC transmission to clients.
//! Serializes all active notification entries so the TUI client can
//! display toast popups with auto-dismiss and progress indicators.

use {
    crate::{KIND, NotificationLevel, NotificationState},
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
};

/// Bridge for notification state.
///
/// Reads [`NotificationState`] from the client's `ExtensionMap` and serializes
/// all entries to JSON. The TUI client deduplicates by `id` and manages
/// display lifecycle (timeouts, stacking, dismissal) independently.
///
/// # Scope: Client
///
/// Uses `Client` scope (per-client `ExtensionMap`) rather than `Shared`
/// because:
/// - Simplifies state ownership (no shared mutable state across clients)
/// - Each client manages its own notification display lifecycle
/// - Producers push to the active client's extension map via `ExtensionApi`
/// - `Shared` scope is not yet implemented in the bridge system
pub struct NotificationBridge;

impl ExtensionStateBridge for NotificationBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<NotificationState>()?;
        if !state.is_active() {
            return Some(serde_json::json!({
                "active": false,
                "entries": [],
            }));
        }
        let entries: Vec<serde_json::Value> = state
            .entries()
            .iter()
            .map(|entry| {
                let mut obj = serde_json::json!({
                    "id": entry.id,
                    "level": level_str(entry.level),
                    "title": entry.title,
                    "body": entry.body,
                });
                if let Some(ref progress) = entry.progress {
                    obj["progress"] = serde_json::json!({
                        "percent": progress.percent,
                        "detail": progress.detail,
                    });
                }
                if let Some(ref source) = entry.source {
                    obj["source"] = serde_json::json!(source);
                }
                obj
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "entries": entries,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<NotificationState>()
            .is_some_and(NotificationState::is_active)
    }
}

/// Convert notification level to a wire-format string.
const fn level_str(level: NotificationLevel) -> &'static str {
    match level {
        NotificationLevel::Info => "info",
        NotificationLevel::Success => "success",
        NotificationLevel::Warning => "warning",
        NotificationLevel::Error => "error",
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
