use super::*;

#[test]
fn test_toggle_visibility() {
    let mut state = LogPanelState::new();
    assert!(!state.visible);

    let visible = state.toggle();
    assert!(visible);
    assert!(state.visible);

    let visible = state.toggle();
    assert!(!visible);
    assert!(!state.visible);
}

#[test]
fn test_scroll_bounds() {
    let mut state = LogPanelState::new();
    state.visible = true;

    // Scroll up
    state.scroll_up(5);
    assert_eq!(state.scroll_offset, 5);
    assert!(!state.auto_scroll);

    // Scroll down
    state.scroll_down(3);
    assert_eq!(state.scroll_offset, 2);

    // Scroll past bottom
    state.scroll_down(10);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.auto_scroll);
}

#[test]
fn test_auto_scroll_behavior() {
    let mut state = LogPanelState::new();
    state.visible = true;

    // Initially auto_scroll is on
    assert!(state.auto_scroll);

    // Scrolling up disables auto_scroll
    state.scroll_up(1);
    assert!(!state.auto_scroll);

    // Scrolling to bottom re-enables it
    state.scroll_to_bottom();
    assert!(state.auto_scroll);
}

#[test]
fn test_level_filter() {
    let mut state = LogPanelState::new();

    // No filter - all pass
    assert!(state.passes_filter(LogLevel::Trace));
    assert!(state.passes_filter(LogLevel::Error));

    // Set warn filter
    state.set_level_filter(Some(LogLevel::Warn));
    assert!(!state.passes_filter(LogLevel::Info));
    assert!(state.passes_filter(LogLevel::Warn));
    assert!(state.passes_filter(LogLevel::Error));
}

#[test]
fn test_scroll_when_buffer_empty() {
    let mut state = LogPanelState::new();
    state.visible = true;

    // Should not panic
    state.scroll_up(10);
    assert_eq!(state.scroll_offset, 10);

    state.scroll_down(100);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_scroll_when_buffer_smaller_than_panel() {
    let mut state = LogPanelState::new();
    state.height = 10;
    state.visible = true;

    // Scroll to top with 5 entries (less than panel height)
    state.scroll_to_top(5);
    assert_eq!(state.scroll_offset, 0); // Can't scroll past visible

    // Scroll to top with 15 entries
    state.scroll_to_top(15);
    assert_eq!(state.scroll_offset, 5); // 15 - 10 = 5
}

#[test]
fn test_level_filter_from_key() {
    let mut state = LogPanelState::new();

    state.set_level_filter_from_key('1');
    assert_eq!(state.level_filter, Some(LogLevel::Error));

    state.set_level_filter_from_key('3');
    assert_eq!(state.level_filter, Some(LogLevel::Info));

    state.set_level_filter_from_key('5');
    assert_eq!(state.level_filter, None);

    // Invalid key should not change filter
    state.set_level_filter_from_key('9');
    assert_eq!(state.level_filter, None);
}

#[test]
fn test_level_filter_from_key_all_keys() {
    let mut state = LogPanelState::new();

    state.set_level_filter_from_key('2');
    assert_eq!(state.level_filter, Some(LogLevel::Warn));

    state.set_level_filter_from_key('4');
    assert_eq!(state.level_filter, Some(LogLevel::Debug));
}

#[test]
fn test_show_when_hidden() {
    let mut state = LogPanelState::new();
    assert!(!state.visible);

    state.show();
    assert!(state.visible);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.auto_scroll);
}

#[test]
fn test_show_when_already_visible() {
    let mut state = LogPanelState::new();
    state.visible = true;
    state.scroll_offset = 5;
    state.auto_scroll = false;

    state.show();
    // Should not reset since already visible
    assert!(state.visible);
    assert_eq!(state.scroll_offset, 5);
    assert!(!state.auto_scroll);
}

#[test]
fn test_hide() {
    let mut state = LogPanelState::new();
    state.visible = true;
    state.hide();
    assert!(!state.visible);
}

#[test]
fn test_set_height_minimum() {
    let mut state = LogPanelState::new();
    state.set_height(1); // Below minimum
    assert_eq!(state.height, 3); // Clamped to minimum

    state.set_height(0);
    assert_eq!(state.height, 3);
}

#[test]
fn test_set_height_normal() {
    let mut state = LogPanelState::new();
    state.set_height(20);
    assert_eq!(state.height, 20);
}

#[test]
fn test_on_new_entries_auto_scroll() {
    let mut state = LogPanelState::new();
    state.auto_scroll = true;
    state.scroll_offset = 5; // Somehow got offset

    state.on_new_entries();
    assert_eq!(state.scroll_offset, 0); // Reset to bottom
}

#[test]
fn test_on_new_entries_manual_scroll() {
    let mut state = LogPanelState::new();
    state.auto_scroll = false;
    state.scroll_offset = 5;

    state.on_new_entries();
    assert_eq!(state.scroll_offset, 5); // Preserved
}

#[test]
fn test_passes_filter_trace_with_no_filter() {
    let state = LogPanelState::new();
    assert!(state.passes_filter(LogLevel::Trace));
}

#[test]
fn test_passes_filter_trace_with_info_filter() {
    let mut state = LogPanelState::new();
    state.set_level_filter(Some(LogLevel::Info));
    assert!(!state.passes_filter(LogLevel::Trace));
    assert!(!state.passes_filter(LogLevel::Debug));
    assert!(state.passes_filter(LogLevel::Info));
    assert!(state.passes_filter(LogLevel::Warn));
    assert!(state.passes_filter(LogLevel::Error));
}

#[test]
fn test_toggle_resets_scroll() {
    let mut state = LogPanelState::new();
    state.scroll_offset = 10;
    state.auto_scroll = false;

    state.toggle(); // Show
    assert!(state.visible);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.auto_scroll);
}

#[test]
fn test_default_impl() {
    let state = LogPanelState::default();
    assert!(!state.visible);
    assert_eq!(state.height, DEFAULT_PANEL_HEIGHT);
    assert!(state.auto_scroll);
}

#[test]
fn test_set_level_filter_resets_scroll() {
    let mut state = LogPanelState::new();
    state.scroll_offset = 10;
    state.auto_scroll = false;

    state.set_level_filter(Some(LogLevel::Error));
    assert_eq!(state.scroll_offset, 0);
    assert!(state.auto_scroll);
}

#[test]
fn test_debug_impl() {
    let state = LogPanelState::new();
    let debug = format!("{state:?}");
    assert!(debug.contains("LogPanelState"));
}
