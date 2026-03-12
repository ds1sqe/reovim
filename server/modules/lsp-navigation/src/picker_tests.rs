use std::path::PathBuf;

use super::*;

fn services() -> ServiceRegistry {
    ServiceRegistry::new()
}

#[test]
fn name_and_title() {
    let picker = LspLocationPicker::new();
    assert_eq!(picker.name(), "lsp-locations");
    assert_eq!(picker.title(), "LSP Locations");
}

#[test]
fn prompt_value() {
    let picker = LspLocationPicker::new();
    assert_eq!(picker.prompt(), "> ");
}

#[test]
fn is_static_true() {
    let picker = LspLocationPicker::new();
    assert!(picker.is_static());
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn default_impl() {
    let picker = LspLocationPicker::default();
    assert_eq!(picker.name(), "lsp-locations");
}

#[test]
fn items_returns_empty() {
    let picker = LspLocationPicker::new();
    let ctx = PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    };
    assert!(picker.items(&ctx, &services()).is_empty());
}

#[test]
fn on_select_goto_location() {
    let picker = LspLocationPicker::new();
    let item = PickerItem {
        display: "test.rs:10:5".to_owned(),
        detail: None,
        data: PickerData::GotoLocation {
            path: PathBuf::from("test.rs"),
            line: 10,
            col: 5,
        },
        icon: None,
    };
    let action = picker.on_select(&item);
    assert!(
        matches!(action, PickerAction::GotoLocation { ref path, line: 10, col: 5 } if *path == Path::new("test.rs"))
    );
}

#[test]
fn on_select_wrong_data_closes() {
    let picker = LspLocationPicker::new();
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::Text("wrong".to_owned()),
        icon: None,
    };
    assert!(matches!(picker.on_select(&item), PickerAction::Close));
}

#[test]
fn preview_goto_location() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let path = dir.path().join("test.rs");
    let content = (1..=20)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, &content).expect("Failed to write file");

    let picker = LspLocationPicker::new();
    let item = PickerItem {
        display: "test.rs:10:line 10".to_owned(),
        detail: None,
        data: PickerData::GotoLocation {
            path: path.clone(),
            line: 10,
            col: 1,
        },
        icon: None,
    };
    let preview = picker.preview(&item, &services());
    assert!(preview.is_some());
    let preview = preview.unwrap();
    assert!(preview.highlight_line.is_some());
    assert_eq!(preview.file_path, Some(path));
}

#[test]
fn preview_wrong_data() {
    let picker = LspLocationPicker::new();
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::Text("wrong".to_owned()),
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

#[test]
fn preview_nonexistent_file() {
    let picker = LspLocationPicker::new();
    let item = PickerItem {
        display: "nope:1:x".to_owned(),
        detail: None,
        data: PickerData::GotoLocation {
            path: PathBuf::from("/nonexistent_lsp_nav_12345.rs"),
            line: 1,
            col: 1,
        },
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

#[test]
fn preview_line_beyond_file_end() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let path = dir.path().join("short.rs");
    std::fs::write(&path, "line 1\nline 2\n").expect("Failed to write");

    let picker = LspLocationPicker::new();
    let item = PickerItem {
        display: "short.rs:9999:x".to_owned(),
        detail: None,
        data: PickerData::GotoLocation {
            path,
            line: 9999,
            col: 1,
        },
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}
