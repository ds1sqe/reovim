use super::*;

fn sample_modules() -> Vec<ModuleEntry> {
    vec![
        ModuleEntry {
            id: "vim".into(),
            version: "0.10.0".into(),
            status: ModuleStatus::Loaded,
            reason: None,
        },
        ModuleEntry {
            id: "emacs".into(),
            version: "0.1.0".into(),
            status: ModuleStatus::Disabled,
            reason: None,
        },
        ModuleEntry {
            id: "broken".into(),
            version: "0.0.1".into(),
            status: ModuleStatus::Failed,
            reason: Some("init error".into()),
        },
    ]
}

#[test]
fn test_new_state_inactive() {
    let state = ModuleManagerState::new();
    assert!(!state.active);
    assert!(state.modules.is_empty());
    assert_eq!(state.selected, 0);
    assert_eq!(state.filter, ModuleFilter::All);
    assert!(!state.detail_visible);
}

#[test]
fn test_default_matches_new() {
    let state = ModuleManagerState::default();
    assert!(!state.active);
}

#[test]
fn test_filtered_all() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    assert_eq!(state.filtered().len(), 3);
}

#[test]
fn test_filtered_loaded() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    state.filter = ModuleFilter::Loaded;
    let filtered = state.filtered();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "vim");
}

#[test]
fn test_filtered_disabled() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    state.filter = ModuleFilter::Disabled;
    let filtered = state.filtered();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "emacs");
}

#[test]
fn test_filtered_failed() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    state.filter = ModuleFilter::Failed;
    let filtered = state.filtered();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "broken");
}

#[test]
fn test_next_wraps_around() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    assert_eq!(state.selected, 0);
    state.next();
    assert_eq!(state.selected, 1);
    state.next();
    assert_eq!(state.selected, 2);
    state.next();
    assert_eq!(state.selected, 0); // wrap
}

#[test]
fn test_prev_wraps_around() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    assert_eq!(state.selected, 0);
    state.prev();
    assert_eq!(state.selected, 2); // wrap to end
    state.prev();
    assert_eq!(state.selected, 1);
}

#[test]
fn test_next_empty_list() {
    let mut state = ModuleManagerState::new();
    state.next(); // should not panic
    assert_eq!(state.selected, 0);
}

#[test]
fn test_prev_empty_list() {
    let mut state = ModuleManagerState::new();
    state.prev(); // should not panic
    assert_eq!(state.selected, 0);
}

#[test]
fn test_toggle_filter_cycles() {
    let mut state = ModuleManagerState::new();
    assert_eq!(state.filter, ModuleFilter::All);
    state.toggle_filter();
    assert_eq!(state.filter, ModuleFilter::Loaded);
    state.toggle_filter();
    assert_eq!(state.filter, ModuleFilter::Disabled);
    state.toggle_filter();
    assert_eq!(state.filter, ModuleFilter::Failed);
    state.toggle_filter();
    assert_eq!(state.filter, ModuleFilter::All);
}

#[test]
fn test_toggle_filter_resets_selection() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    state.selected = 2;
    state.toggle_filter();
    assert_eq!(state.selected, 0);
}

#[test]
fn test_toggle_detail() {
    let mut state = ModuleManagerState::new();
    assert!(!state.detail_visible);
    state.toggle_detail();
    assert!(state.detail_visible);
    state.toggle_detail();
    assert!(!state.detail_visible);
}

#[test]
fn test_selected_entry() {
    let mut state = ModuleManagerState::new();
    state.modules = sample_modules();
    let entry = state.selected_entry().unwrap();
    assert_eq!(entry.id, "vim");
}

#[test]
fn test_selected_entry_empty() {
    let state = ModuleManagerState::new();
    assert!(state.selected_entry().is_none());
}

#[test]
fn test_module_status_indicator() {
    assert_eq!(ModuleStatus::Loaded.indicator(), "[*]");
    assert_eq!(ModuleStatus::Disabled.indicator(), "[-]");
    assert_eq!(ModuleStatus::Failed.indicator(), "[!]");
}

#[test]
fn test_module_status_label() {
    assert_eq!(ModuleStatus::Loaded.label(), "loaded");
    assert_eq!(ModuleStatus::Disabled.label(), "disabled");
    assert_eq!(ModuleStatus::Failed.label(), "failed");
}

#[test]
fn test_filter_label() {
    assert_eq!(ModuleFilter::All.label(), "All");
    assert_eq!(ModuleFilter::Loaded.label(), "Loaded");
    assert_eq!(ModuleFilter::Disabled.label(), "Disabled");
    assert_eq!(ModuleFilter::Failed.label(), "Failed");
}

#[test]
fn test_session_extension_create() {
    let state = <ModuleManagerState as SessionExtension>::create();
    assert!(!state.active);
}
