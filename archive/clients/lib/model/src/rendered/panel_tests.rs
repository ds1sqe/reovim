use super::*;

#[test]
fn test_panel_state_new() {
    let state = PanelState::new(1, 100);
    assert_eq!(state.viewport_id, 1);
    assert_eq!(state.buffer_id, 100);
    assert_eq!(state.visible_start, 0);
    assert_eq!(state.visible_end, 0);
}

#[test]
fn test_panel_state_builder() {
    let state = PanelState::new(1, 100)
        .with_visible_range(10, 30)
        .with_cursor(15, 5)
        .with_total_lines(100);

    assert_eq!(state.visible_start, 10);
    assert_eq!(state.visible_end, 30);
    assert_eq!(state.cursor_line, 15);
    assert_eq!(state.cursor_col, 5);
    assert_eq!(state.total_lines, 100);
}

#[test]
fn test_is_line_visible() {
    let state = PanelState::new(1, 100).with_visible_range(10, 30);

    assert!(!state.is_line_visible(9)); // Above visible
    assert!(state.is_line_visible(10)); // First visible
    assert!(state.is_line_visible(20)); // Middle
    assert!(state.is_line_visible(30)); // Last visible
    assert!(!state.is_line_visible(31)); // Below visible
}

#[test]
fn test_visible_line_count() {
    let state = PanelState::new(1, 100).with_visible_range(10, 30);
    assert_eq!(state.visible_line_count(), 21); // 10..=30 inclusive
}

#[test]
fn test_is_cursor_visible() {
    let state = PanelState::new(1, 100)
        .with_visible_range(10, 30)
        .with_cursor(20, 0);
    assert!(state.is_cursor_visible());

    let state = state.with_cursor(5, 0);
    assert!(!state.is_cursor_visible());
}

#[test]
fn test_scroll_percentage() {
    // 100 lines total, showing 20 lines (0-19)
    let state = PanelState::new(1, 100)
        .with_visible_range(0, 19)
        .with_total_lines(100);
    assert!((state.scroll_percentage() - 0.0).abs() < f32::EPSILON);

    // Scrolled to middle
    let state = state.with_visible_range(40, 59);
    assert!((state.scroll_percentage() - 0.5).abs() < f32::EPSILON);

    // Scrolled to end
    let state = state.with_visible_range(80, 99);
    assert!((state.scroll_percentage() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_is_at_top_bottom() {
    let state = PanelState::new(1, 100)
        .with_visible_range(0, 19)
        .with_total_lines(100);
    assert!(state.is_at_top());
    assert!(!state.is_at_bottom());

    let state = state.with_visible_range(80, 99);
    assert!(!state.is_at_top());
    assert!(state.is_at_bottom());
}

#[test]
fn test_visible_line_count_inverted_range() {
    // When end < start, count should be 0
    let mut state = PanelState::new(1, 100);
    state.visible_start = 30;
    state.visible_end = 10;
    assert_eq!(state.visible_line_count(), 0);
}

#[test]
fn test_scroll_percentage_zero_total_lines() {
    let state = PanelState::new(1, 100).with_total_lines(0);
    assert!((state.scroll_percentage() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn test_scroll_percentage_all_visible() {
    // When all lines fit in view, max_start = 0
    let state = PanelState::new(1, 100)
        .with_visible_range(0, 9)
        .with_total_lines(10);
    assert!((state.scroll_percentage() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn test_visible_range() {
    let state = PanelState::new(1, 100).with_visible_range(5, 25);
    assert_eq!(state.visible_range(), (5, 25));
}

#[test]
fn test_visible_range_inclusive() {
    let state = PanelState::new(1, 100).with_visible_range(10, 30);
    let range = state.visible_range_inclusive();
    assert_eq!(*range.start(), 10);
    assert_eq!(*range.end(), 30);
}
