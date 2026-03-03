//! Fold extension state bridge.
//!
//! Adapts [`FoldSessionState`] to JSON for gRPC transmission to clients.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use super::state::FoldSessionState;

/// Bridge for fold state.
///
/// Reads `FoldSessionState` from the session's `ExtensionMap` and serializes
/// collapsed folds to JSON. Scope is `Shared` since folds are per-buffer
/// (shared across all clients in a session).
pub struct FoldBridge;

impl ExtensionStateBridge for FoldBridge {
    fn kind(&self) -> &'static str {
        "range-finder-fold"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Shared
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<FoldSessionState>()?;
        if !state.has_collapsed_folds() {
            return None;
        }

        let mut folds = serde_json::Map::new();

        for (buf_id, fold) in state.buffers() {
            if !fold.has_collapsed() {
                continue;
            }
            let collapsed: Vec<serde_json::Value> = fold
                .collapsed_info()
                .map(|(start_line, hidden_count, preview)| {
                    serde_json::json!({
                        "start_line": start_line,
                        "hidden_count": hidden_count,
                        "preview": preview,
                    })
                })
                .collect();

            folds.insert(buf_id.to_string(), serde_json::Value::Array(collapsed));
        }

        Some(serde_json::json!({ "folds": folds }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<FoldSessionState>()
            .is_some_and(FoldSessionState::has_collapsed_folds)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_syntax::{FoldKind, FoldRange},
        reovim_kernel::api::v1::BufferId,
    };

    fn buffer_id(n: usize) -> BufferId {
        BufferId::from_raw(n)
    }

    #[test]
    fn test_fold_bridge_kind() {
        assert_eq!(FoldBridge.kind(), "range-finder-fold");
    }

    #[test]
    fn test_fold_bridge_scope() {
        assert_eq!(FoldBridge.scope(), ExtensionScope::Shared);
    }

    #[test]
    fn test_fold_bridge_snapshot_no_folds() {
        let map = ExtensionMap::new();
        // No FoldSessionState in map
        assert!(FoldBridge.snapshot(&map).is_none());
    }

    #[test]
    fn test_fold_bridge_snapshot_no_collapsed() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<FoldSessionState>();
        let fold = state.get_or_insert(buffer_id(1));
        fold.set_ranges(vec![FoldRange::new(0, 5, FoldKind::Function, "fn main() {")]);
        // All folds open, no collapsed
        assert!(FoldBridge.snapshot(&map).is_none());
    }

    #[test]
    fn test_fold_bridge_snapshot_with_collapsed() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<FoldSessionState>();
        let fold = state.get_or_insert(buffer_id(1));
        fold.set_ranges(vec![
            FoldRange::new(2, 8, FoldKind::Function, "fn foo() {"),
            FoldRange::new(10, 15, FoldKind::Class, "impl Bar {"),
        ]);
        fold.close(0); // Collapse fn foo

        let snap = FoldBridge.snapshot(&map).unwrap();
        let folds = snap["folds"].as_object().unwrap();
        assert!(folds.contains_key("1")); // buffer_id 1

        let collapsed = folds["1"].as_array().unwrap();
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0]["start_line"], 2);
        assert_eq!(collapsed[0]["hidden_count"], 6); // lines 3-8
        assert_eq!(collapsed[0]["preview"], "fn foo() {");
    }

    #[test]
    fn test_fold_bridge_snapshot_multi_buffer_mixed() {
        let mut map = ExtensionMap::new();
        let session_state = map.get_or_insert::<FoldSessionState>();

        // Buffer 1: has folds but none collapsed
        let buf1 = session_state.get_or_insert(buffer_id(1));
        buf1.set_ranges(vec![FoldRange::new(0, 5, FoldKind::Function, "fn open() {")]);

        // Buffer 2: has a collapsed fold
        let buf2 = session_state.get_or_insert(buffer_id(2));
        buf2.set_ranges(vec![FoldRange::new(1, 4, FoldKind::Function, "fn closed() {")]);
        buf2.close(0);

        let snap = FoldBridge.snapshot(&map).unwrap();
        let result = snap["folds"].as_object().unwrap();
        // Only buffer 2 should appear (buffer 1 skipped via continue)
        assert!(!result.contains_key("1"));
        assert!(result.contains_key("2"));
    }

    #[test]
    fn test_fold_bridge_is_active_empty() {
        let map = ExtensionMap::new();
        assert!(!FoldBridge.is_active(&map));
    }

    #[test]
    fn test_fold_bridge_is_active_true() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<FoldSessionState>();
        let fold = state.get_or_insert(buffer_id(1));
        fold.set_ranges(vec![FoldRange::new(0, 5, FoldKind::Function, "fn main() {")]);
        fold.close(0);
        assert!(FoldBridge.is_active(&map));
    }

    #[test]
    fn test_fold_bridge_is_active_false() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<FoldSessionState>();
        assert!(!FoldBridge.is_active(&map));
    }
}
