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
