use super::*;

use reovim_driver_text_session::ExtensionMap;

use crate::state::{FlatItem, SettingKind, SettingsState};

#[test]
fn test_bridge_kind() {
    let bridge = SettingsBridge;
    assert_eq!(bridge.kind(), "settings");
}

#[test]
fn test_bridge_scope() {
    let bridge = SettingsBridge;
    assert_eq!(bridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_bridge_is_active_no_state() {
    let bridge = SettingsBridge;
    let extensions = ExtensionMap::new();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_closed() {
    let bridge = SettingsBridge;
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<SettingsState>();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_open() {
    let bridge = SettingsBridge;
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<SettingsState>();
    state.open = true;
    assert!(bridge.is_active(&extensions));
}

#[test]
fn test_bridge_snapshot_no_state() {
    let bridge = SettingsBridge;
    let extensions = ExtensionMap::new();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_snapshot_closed() {
    let bridge = SettingsBridge;
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<SettingsState>();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_snapshot_open_empty() {
    let bridge = SettingsBridge;
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<SettingsState>();
    state.open = true;

    let snap = bridge
        .snapshot(&extensions)
        .expect("should produce snapshot");
    assert_eq!(snap["open"], true);
    let items = snap["items"].as_array().expect("items array");
    assert!(items.is_empty());
}

#[test]
fn test_bridge_snapshot_with_items() {
    let bridge = SettingsBridge;
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<SettingsState>();
    state.open = true;
    state.items.push(FlatItem::SectionHeader {
        title: "Editor".to_string(),
    });
    state.items.push(FlatItem::Setting {
        name: "tabstop".to_string(),
        description: "Tab size".to_string(),
        value: "4".to_string(),
        kind: SettingKind::Int,
    });
    state.selected_index = 1;

    let snap = bridge
        .snapshot(&extensions)
        .expect("should produce snapshot");
    let items = snap["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["type"], "header");
    assert_eq!(items[0]["title"], "Editor");
    assert_eq!(items[1]["type"], "setting");
    assert_eq!(items[1]["name"], "tabstop");
    assert_eq!(items[1]["kind"], "int");
    assert_eq!(snap["selected_index"], 1);
}

#[test]
fn test_kind_to_str() {
    assert_eq!(kind_to_str(SettingKind::Bool), "bool");
    assert_eq!(kind_to_str(SettingKind::Int), "int");
    assert_eq!(kind_to_str(SettingKind::String), "string");
    assert_eq!(kind_to_str(SettingKind::Choice), "choice");
}
