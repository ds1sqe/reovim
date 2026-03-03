//! Completion extension state bridge.
//!
//! Serializes [`CompletionState`] to JSON for gRPC transmission to clients.
//! Both TUI (Rust) and Web (TypeScript) extensions consume the same JSON.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::state::CompletionState;

/// Bridge for completion popup state.
///
/// Reads [`CompletionState`] from the client's `ExtensionMap` and serializes
/// it to JSON with fields matching what TUI and Web extensions expect.
pub struct CompletionBridge;

impl ExtensionStateBridge for CompletionBridge {
    fn kind(&self) -> &'static str {
        "completion"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<CompletionState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        let items: Vec<serde_json::Value> = state
            .items
            .iter()
            .map(|item| {
                let mut obj = serde_json::json!({
                    "label": item.label,
                    "kindAbbrev": item.kind_abbrev,
                    "sourceId": item.source_id,
                });
                if let Some(ref detail) = item.detail {
                    obj["detail"] = serde_json::json!(detail);
                }
                obj
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "items": items,
            "selected": state.selected,
            "prefix": state.prefix,
            "scrollOffset": state.scroll_offset,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<CompletionState>()
            .is_some_and(|s| s.active)
    }
}

#[cfg(test)]
mod tests {
    use reovim_driver_completion::CompletionKind;

    use {super::*, crate::state::CompletionItemSnapshot};

    fn make_snapshot(label: &str, kind: CompletionKind) -> CompletionItemSnapshot {
        CompletionItemSnapshot {
            label: label.to_owned(),
            kind_abbrev: kind.abbreviation().to_owned(),
            kind,
            detail: None,
            source_id: "test".to_owned(),
        }
    }

    #[test]
    fn bridge_kind() {
        assert_eq!(CompletionBridge.kind(), "completion");
    }

    #[test]
    fn bridge_scope() {
        assert_eq!(CompletionBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn snapshot_no_state_returns_none() {
        let map = ExtensionMap::new();
        assert!(CompletionBridge.snapshot(&map).is_none());
    }

    #[test]
    fn snapshot_inactive() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<CompletionState>();

        let snap = CompletionBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
        // Inactive snapshot should be minimal.
        assert!(snap.get("items").is_none());
    }

    #[test]
    fn snapshot_active_empty() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CompletionState>();
        state.active = true;
        state.prefix = "fo".to_owned();

        let snap = CompletionBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert_eq!(snap["prefix"], "fo");
        assert_eq!(snap["selected"], 0);
        assert_eq!(snap["scrollOffset"], 0);
        assert!(snap["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn snapshot_active_with_items() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CompletionState>();
        state.active = true;
        state.prefix = "pr".to_owned();
        state.selected = 1;
        state.items = vec![
            CompletionItemSnapshot {
                label: "println".to_owned(),
                kind_abbrev: "fn".to_owned(),
                kind: CompletionKind::Function,
                detail: Some("macro".to_owned()),
                source_id: "lsp".to_owned(),
            },
            make_snapshot("print", CompletionKind::Function),
        ];

        let snap = CompletionBridge.snapshot(&map).unwrap();
        assert_eq!(snap["selected"], 1);
        assert_eq!(snap["prefix"], "pr");

        let items = snap["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["label"], "println");
        assert_eq!(items[0]["kindAbbrev"], "fn");
        assert_eq!(items[0]["sourceId"], "lsp");
        assert_eq!(items[0]["detail"], "macro");
        assert_eq!(items[1]["label"], "print");
        assert!(items[1].get("detail").is_none());
    }

    #[test]
    fn is_active_no_state() {
        let map = ExtensionMap::new();
        assert!(!CompletionBridge.is_active(&map));
    }

    #[test]
    fn is_active_inactive_state() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<CompletionState>();
        assert!(!CompletionBridge.is_active(&map));
    }

    #[test]
    fn is_active_active_state() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<CompletionState>();
        state.active = true;
        assert!(CompletionBridge.is_active(&map));
    }
}
