use super::*;

#[test]
fn picker_item_construction() {
    let item = PickerItem {
        display: "main.rs".to_owned(),
        detail: Some("src/main.rs".to_owned()),
        data: PickerData::FilePath(PathBuf::from("src/main.rs")),
        icon: Some('\u{f15b}'),
    };
    assert_eq!(item.display, "main.rs");
    assert_eq!(item.detail.as_deref(), Some("src/main.rs"));
    assert_eq!(item.icon, Some('\u{f15b}'));
}

#[test]
fn picker_item_no_detail_no_icon() {
    let item = PickerItem {
        display: "test".to_owned(),
        detail: None,
        data: PickerData::Text("hello".to_owned()),
        icon: None,
    };
    assert!(item.detail.is_none());
    assert!(item.icon.is_none());
}

#[test]
fn picker_item_clone() {
    let item = PickerItem {
        display: "foo".to_owned(),
        detail: Some("bar".to_owned()),
        data: PickerData::BufferId(42),
        icon: Some('B'),
    };
    #[allow(clippy::redundant_clone)]
    let cloned = item.clone();
    assert_eq!(cloned.display, "foo");
    assert_eq!(cloned.detail.as_deref(), Some("bar"));
}

#[test]
fn picker_item_debug() {
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::Text("y".to_owned()),
        icon: None,
    };
    let debug = format!("{item:?}");
    assert!(debug.contains("PickerItem"));
}

#[test]
fn picker_data_file_path() {
    let data = PickerData::FilePath(PathBuf::from("/tmp/test.rs"));
    let debug = format!("{data:?}");
    assert!(debug.contains("FilePath"));
}

#[test]
fn picker_data_buffer_id() {
    let data = PickerData::BufferId(7);
    let debug = format!("{data:?}");
    assert!(debug.contains("BufferId"));
}

#[test]
fn picker_data_command() {
    let data = PickerData::Command("editor:save".to_owned());
    let debug = format!("{data:?}");
    assert!(debug.contains("Command"));
}

#[test]
fn picker_data_goto_location() {
    let data = PickerData::GotoLocation {
        path: PathBuf::from("src/lib.rs"),
        line: 10,
        col: 5,
    };
    let debug = format!("{data:?}");
    assert!(debug.contains("GotoLocation"));
}

#[test]
fn picker_data_text() {
    let data = PickerData::Text("hello".to_owned());
    let debug = format!("{data:?}");
    assert!(debug.contains("Text"));
}

#[test]
fn picker_data_clone() {
    let data = PickerData::GotoLocation {
        path: PathBuf::from("a.rs"),
        line: 1,
        col: 2,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = data.clone();
    let debug = format!("{cloned:?}");
    assert!(debug.contains("GotoLocation"));
}
