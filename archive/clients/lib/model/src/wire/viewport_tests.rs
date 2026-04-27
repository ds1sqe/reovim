use super::*;

#[test]
fn test_viewport_state_new() {
    let state = ViewportState::new(1, 100);
    assert_eq!(state.id, 1);
    assert_eq!(state.buffer_id, 100);
    assert_eq!(state.top_line, 0);
    assert_eq!(state.cursor_line, 0);
}

#[test]
fn test_viewport_state_builder() {
    let state = ViewportState::new(1, 100)
        .with_cursor(10, 5)
        .with_scroll(8, 0);

    assert_eq!(state.cursor_line, 10);
    assert_eq!(state.cursor_col, 5);
    assert_eq!(state.top_line, 8);
}

#[test]
fn test_is_line_visible() {
    let state = ViewportState::new(1, 100).with_scroll(10, 0);

    // Visible lines: 10-29 (20 lines visible)
    assert!(!state.is_line_visible(9, 20)); // Above viewport
    assert!(state.is_line_visible(10, 20)); // First visible
    assert!(state.is_line_visible(20, 20)); // Middle
    assert!(state.is_line_visible(29, 20)); // Last visible
    assert!(!state.is_line_visible(30, 20)); // Below viewport
}

#[test]
fn test_viewport_update_empty() {
    let update = ViewportUpdate::new(1);
    assert!(update.is_empty());
}

#[test]
fn test_viewport_update_scroll() {
    let update = ViewportUpdate::new(1).scroll_to(100, 10);
    assert!(!update.is_empty());
    assert_eq!(update.top_line, Some(100));
    assert_eq!(update.left_col, Some(10));
}

#[test]
fn test_viewport_update_apply() {
    let mut state = ViewportState::new(1, 100);
    let update = ViewportUpdate::new(1).scroll_to(50, 0).cursor_to(55, 10);

    update.apply_to(&mut state);

    assert_eq!(state.top_line, 50);
    assert_eq!(state.cursor_line, 55);
    assert_eq!(state.cursor_col, 10);
}
