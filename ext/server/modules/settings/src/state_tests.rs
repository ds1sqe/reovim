use super::*;

fn sample_items() -> Vec<FlatItem> {
    vec![
        FlatItem::SectionHeader {
            title: "Editor".to_string(),
        },
        FlatItem::Setting {
            name: "tabstop".to_string(),
            description: "Number of spaces per tab".to_string(),
            value: "4".to_string(),
            kind: SettingKind::Int,
        },
        FlatItem::Setting {
            name: "expandtab".to_string(),
            description: "Use spaces instead of tabs".to_string(),
            value: "true".to_string(),
            kind: SettingKind::Bool,
        },
        FlatItem::SectionHeader {
            title: "Display".to_string(),
        },
        FlatItem::Setting {
            name: "number".to_string(),
            description: "Show line numbers".to_string(),
            value: "true".to_string(),
            kind: SettingKind::Bool,
        },
    ]
}

#[test]
fn test_state_default() {
    let state = SettingsState::default();
    assert!(!state.open);
    assert!(state.items.is_empty());
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_state_create() {
    let state = SettingsState::create();
    assert!(!state.open);
}

#[test]
fn test_state_debug() {
    let state = SettingsState::default();
    let debug = format!("{state:?}");
    assert!(debug.contains("SettingsState"));
}

#[test]
fn test_flat_item_section_header() {
    let item = FlatItem::SectionHeader {
        title: "Editor".to_string(),
    };
    assert!(matches!(item, FlatItem::SectionHeader { .. }));
}

#[test]
fn test_flat_item_setting() {
    let item = FlatItem::Setting {
        name: "tabstop".to_string(),
        description: "Tab size".to_string(),
        value: "4".to_string(),
        kind: SettingKind::Int,
    };
    assert!(matches!(item, FlatItem::Setting { .. }));
}

#[test]
fn test_flat_item_eq() {
    let a = FlatItem::SectionHeader {
        title: "A".to_string(),
    };
    let b = FlatItem::SectionHeader {
        title: "A".to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn test_flat_item_ne() {
    let a = FlatItem::SectionHeader {
        title: "A".to_string(),
    };
    let b = FlatItem::SectionHeader {
        title: "B".to_string(),
    };
    assert_ne!(a, b);
}

#[test]
fn test_flat_item_clone() {
    let item = FlatItem::Setting {
        name: "x".to_string(),
        description: "y".to_string(),
        value: "z".to_string(),
        kind: SettingKind::String,
    };
    let cloned = item.clone();
    assert_eq!(item, cloned);
}

#[test]
fn test_flat_item_debug() {
    let item = FlatItem::SectionHeader {
        title: "Test".to_string(),
    };
    let debug = format!("{item:?}");
    assert!(debug.contains("SectionHeader"));
}

#[test]
fn test_setting_kind_eq() {
    assert_eq!(SettingKind::Bool, SettingKind::Bool);
    assert_ne!(SettingKind::Bool, SettingKind::Int);
}

#[test]
fn test_setting_kind_clone() {
    let kind = SettingKind::Choice;
    let cloned = kind;
    assert_eq!(cloned, SettingKind::Choice);
}

#[test]
fn test_setting_kind_debug() {
    let debug = format!("{:?}", SettingKind::String);
    assert!(debug.contains("String"));
}

#[test]
fn test_select_next() {
    let mut state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 1,
        scroll_offset: 0,
    };
    state.select_next();
    assert_eq!(state.selected_index, 2);
}

#[test]
fn test_select_next_skips_header() {
    let mut state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 2,
        scroll_offset: 0,
    };
    // Next after "expandtab" (index 2) should skip header (index 3) to "number" (index 4)
    state.select_next();
    assert_eq!(state.selected_index, 4);
}

#[test]
fn test_select_next_at_end() {
    let mut state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 4,
        scroll_offset: 0,
    };
    state.select_next();
    // Should stay at 4
    assert_eq!(state.selected_index, 4);
}

#[test]
fn test_select_next_empty() {
    let mut state = SettingsState::default();
    state.select_next();
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_select_prev() {
    let mut state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 2,
        scroll_offset: 0,
    };
    state.select_prev();
    assert_eq!(state.selected_index, 1);
}

#[test]
fn test_select_prev_skips_header() {
    let mut state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 4,
        scroll_offset: 0,
    };
    // Prev from "number" (4) should skip header (3) to "expandtab" (2)
    state.select_prev();
    assert_eq!(state.selected_index, 2);
}

#[test]
fn test_select_prev_at_start() {
    let mut state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 0,
        scroll_offset: 0,
    };
    state.select_prev();
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_selected_setting_name() {
    let state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 1,
        scroll_offset: 0,
    };
    assert_eq!(state.selected_setting_name(), Some("tabstop"));
}

#[test]
fn test_selected_setting_name_header() {
    let state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 0,
        scroll_offset: 0,
    };
    assert_eq!(state.selected_setting_name(), None);
}

#[test]
fn test_selected_setting_name_out_of_bounds() {
    let state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 100,
        scroll_offset: 0,
    };
    assert_eq!(state.selected_setting_name(), None);
}

#[test]
fn test_setting_count() {
    let state = SettingsState {
        open: true,
        items: sample_items(),
        selected_index: 0,
        scroll_offset: 0,
    };
    assert_eq!(state.setting_count(), 3);
}

#[test]
fn test_setting_count_empty() {
    let state = SettingsState::default();
    assert_eq!(state.setting_count(), 0);
}
