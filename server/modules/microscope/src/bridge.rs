//! Microscope extension state bridge.
//!
//! Serializes [`MicroscopeState`] to JSON for gRPC transmission to clients.
//! Both TUI (Rust) and Web (TypeScript) extensions consume the same JSON.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::state::MicroscopeState;

/// Bridge for microscope picker state.
///
/// Reads [`MicroscopeState`] from the client's `ExtensionMap` and serializes
/// it to JSON with fields matching what TUI and Web extensions expect.
pub struct MicroscopeBridge;

impl ExtensionStateBridge for MicroscopeBridge {
    fn kind(&self) -> &'static str {
        "microscope"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<MicroscopeState>()?;

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
                    "display": item.display,
                });
                if let Some(ref detail) = item.detail {
                    obj["detail"] = serde_json::json!(detail);
                }
                if let Some(icon) = item.icon {
                    obj["icon"] = serde_json::json!(icon.to_string());
                }
                obj
            })
            .collect();

        let mut json = serde_json::json!({
            "active": true,
            "query": state.query,
            "cursor": state.cursor,
            "selected": state.selected,
            "scrollOffset": state.scroll_offset,
            "pickerName": state.picker_name,
            "pickerTitle": state.picker_title,
            "prompt": state.prompt,
            "items": items,
            "totalCount": state.total_count,
            "matchedCount": state.matched_count,
        });

        if let Some(ref preview) = state.preview {
            let mut preview_json = serde_json::json!({
                "lines": preview.lines,
            });
            if let Some(hl) = preview.highlight_line {
                preview_json["highlightLine"] = serde_json::json!(hl);
            }
            if let Some(ref path) = preview.file_path {
                preview_json["filePath"] = serde_json::json!(path.to_string_lossy());
            }
            json["preview"] = preview_json;
        }

        Some(json)
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<MicroscopeState>()
            .is_some_and(|s| s.active)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use reovim_driver_picker::PreviewContent;

    use {super::*, crate::state::PickerItemSnapshot};

    #[test]
    fn bridge_kind() {
        assert_eq!(MicroscopeBridge.kind(), "microscope");
    }

    #[test]
    fn bridge_scope() {
        assert_eq!(MicroscopeBridge.scope(), ExtensionScope::Client);
    }

    #[test]
    fn snapshot_no_state_returns_none() {
        let map = ExtensionMap::new();
        assert!(MicroscopeBridge.snapshot(&map).is_none());
    }

    #[test]
    fn snapshot_inactive() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<MicroscopeState>();

        let snap = MicroscopeBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
        // Inactive snapshot should be minimal.
        assert!(snap.get("query").is_none());
    }

    #[test]
    fn snapshot_active_empty() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<MicroscopeState>();
        state.active = true;
        state.picker_name = "files".to_owned();
        state.picker_title = "Files".to_owned();

        let snap = MicroscopeBridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], true);
        assert_eq!(snap["query"], "");
        assert_eq!(snap["pickerName"], "files");
        assert_eq!(snap["pickerTitle"], "Files");
        assert_eq!(snap["prompt"], "> ");
        assert!(snap["items"].as_array().unwrap().is_empty());
        assert_eq!(snap["totalCount"], 0);
        assert_eq!(snap["matchedCount"], 0);
        assert!(snap.get("preview").is_none());
    }

    #[test]
    fn snapshot_active_with_items() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<MicroscopeState>();
        state.active = true;
        state.query = "main".to_owned();
        state.cursor = 4;
        state.selected = 1;
        state.total_count = 100;
        state.matched_count = 3;
        state.items = vec![
            PickerItemSnapshot {
                display: "main.rs".to_owned(),
                detail: Some("src/main.rs".to_owned()),
                icon: Some('f'),
            },
            PickerItemSnapshot {
                display: "main.go".to_owned(),
                detail: None,
                icon: None,
            },
        ];

        let snap = MicroscopeBridge.snapshot(&map).unwrap();
        assert_eq!(snap["query"], "main");
        assert_eq!(snap["cursor"], 4);
        assert_eq!(snap["selected"], 1);
        assert_eq!(snap["totalCount"], 100);
        assert_eq!(snap["matchedCount"], 3);

        let items = snap["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["display"], "main.rs");
        assert_eq!(items[0]["detail"], "src/main.rs");
        assert_eq!(items[0]["icon"], "f");
        assert_eq!(items[1]["display"], "main.go");
        assert!(items[1].get("detail").is_none());
        assert!(items[1].get("icon").is_none());
    }

    #[test]
    fn snapshot_active_with_preview() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<MicroscopeState>();
        state.active = true;
        state.preview = Some(PreviewContent {
            lines: vec!["fn main() {".to_owned(), "}".to_owned()],
            highlight_line: Some(0),
            file_path: Some(PathBuf::from("main.rs")),
        });

        let snap = MicroscopeBridge.snapshot(&map).unwrap();
        let preview = &snap["preview"];
        assert_eq!(preview["lines"].as_array().unwrap().len(), 2);
        assert_eq!(preview["highlightLine"], 0);
        assert_eq!(preview["filePath"], "main.rs");
    }

    #[test]
    fn snapshot_preview_without_highlight_or_path() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<MicroscopeState>();
        state.active = true;
        state.preview = Some(PreviewContent {
            lines: vec!["line".to_owned()],
            highlight_line: None,
            file_path: None,
        });

        let snap = MicroscopeBridge.snapshot(&map).unwrap();
        let preview = &snap["preview"];
        assert!(preview.get("highlightLine").is_none());
        assert!(preview.get("filePath").is_none());
    }

    #[test]
    fn is_active_no_state() {
        let map = ExtensionMap::new();
        assert!(!MicroscopeBridge.is_active(&map));
    }

    #[test]
    fn is_active_inactive_state() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<MicroscopeState>();
        assert!(!MicroscopeBridge.is_active(&map));
    }

    #[test]
    fn is_active_active_state() {
        let mut map = ExtensionMap::new();
        let state = map.get_or_insert::<MicroscopeState>();
        state.active = true;
        assert!(MicroscopeBridge.is_active(&map));
    }
}
