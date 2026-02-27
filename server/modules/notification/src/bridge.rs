//! Notification extension state bridge.
//!
//! Adapts [`NotificationState`] to JSON for gRPC transmission to clients.
//! Serializes all active notification entries so the TUI client can
//! display toast popups with auto-dismiss and progress indicators.

use {
    crate::{NotificationLevel, NotificationState},
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
        "notification"
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
mod tests {
    use super::*;

    #[test]
    fn test_bridge_kind() {
        assert_eq!(NotificationBridge.kind(), "notification");
    }

    #[test]
    fn test_bridge_scope() {
        assert_eq!(NotificationBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn test_snapshot_empty_map() {
        let map = ExtensionMap::new();
        assert!(NotificationBridge.snapshot(&map).is_none());
    }

    #[test]
    fn test_snapshot_inactive_state() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<NotificationState>();

        let snap = NotificationBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
        assert!(snap["entries"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_snapshot_with_entries() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<NotificationState>();
        state.push(NotificationLevel::Success, "File saved");
        state.push(NotificationLevel::Error, "Build failed");

        let snap = NotificationBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);

        let entries = snap["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["title"], "File saved");
        assert_eq!(entries[0]["level"], "success");
        assert_eq!(entries[0]["id"], 0);
        assert_eq!(entries[1]["title"], "Build failed");
        assert_eq!(entries[1]["level"], "error");
        assert_eq!(entries[1]["id"], 1);
    }

    #[test]
    fn test_snapshot_with_body() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<NotificationState>();
        state.push_with_body(NotificationLevel::Warning, "Warn", "details");

        let snap = NotificationBridge.snapshot(&map).unwrap();
        let entry = &snap["entries"][0];
        assert_eq!(entry["body"], "details");
    }

    #[test]
    fn test_snapshot_with_progress() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<NotificationState>();
        state.push_progress("Building", 35, "3/10 files");

        let snap = NotificationBridge.snapshot(&map).unwrap();
        let entry = &snap["entries"][0];
        assert_eq!(entry["level"], "info");
        let progress = &entry["progress"];
        assert_eq!(progress["percent"], 35);
        assert_eq!(progress["detail"], "3/10 files");
    }

    #[test]
    fn test_snapshot_without_progress() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<NotificationState>();
        state.push(NotificationLevel::Info, "no progress");

        let snap = NotificationBridge.snapshot(&map).unwrap();
        let entry = &snap["entries"][0];
        // progress field should not be present
        assert!(entry.get("progress").is_none());
    }

    #[test]
    fn test_is_active_empty_map() {
        let map = ExtensionMap::new();
        assert!(!NotificationBridge.is_active(&map));
    }

    #[test]
    fn test_is_active_no_entries() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<NotificationState>();
        assert!(!NotificationBridge.is_active(&map));
    }

    #[test]
    fn test_is_active_with_entries() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<NotificationState>();
        state.push(NotificationLevel::Info, "test");
        assert!(NotificationBridge.is_active(&map));
    }

    #[test]
    fn test_is_active_after_dismiss() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<NotificationState>();
        let id = state.push(NotificationLevel::Info, "test");
        assert!(NotificationBridge.is_active(&map));

        let state = map.get_or_insert::<NotificationState>();
        state.dismiss(id);
        assert!(!NotificationBridge.is_active(&map));
    }

    #[test]
    fn test_snapshot_mixed_progress_and_no_progress() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<NotificationState>();
        state.push(NotificationLevel::Info, "plain");
        state.push_progress("Building", 42, "4/10");

        let snap = NotificationBridge.snapshot(&map).unwrap();
        let entries = snap["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);

        // First entry has no progress
        assert!(entries[0].get("progress").is_none());

        // Second entry has progress
        let progress = &entries[1]["progress"];
        assert_eq!(progress["percent"], 42);
        assert_eq!(progress["detail"], "4/10");
    }

    #[test]
    fn test_level_str_all_variants() {
        assert_eq!(level_str(NotificationLevel::Info), "info");
        assert_eq!(level_str(NotificationLevel::Success), "success");
        assert_eq!(level_str(NotificationLevel::Warning), "warning");
        assert_eq!(level_str(NotificationLevel::Error), "error");
    }
}
