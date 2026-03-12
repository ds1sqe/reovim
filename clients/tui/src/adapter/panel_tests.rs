use super::*;

fn test_view() -> View {
    View::new(BufferId::from_raw(42))
}

fn test_panel() -> TuiPanel {
    TuiPanel::new(test_view(), 100, 24, 1000)
}

#[test]
fn test_panel_new() {
    let panel = test_panel();
    assert_eq!(panel.viewport_id, 100);
    assert_eq!(panel.window_height, 24);
    assert_eq!(panel.total_lines, 1000);
}

#[test]
fn test_panel_buffer_id() {
    let panel = test_panel();
    assert_eq!(panel.buffer_id(), 42);
}

#[test]
fn test_panel_viewport_id() {
    let panel = test_panel();
    assert_eq!(panel.viewport_id(), 100);
}

#[test]
fn test_panel_visible_range_at_top() {
    let panel = test_panel();
    // scroll_top = 0, height = 24, so visible range is 0..=23
    assert_eq!(panel.visible_range(), 0..=23);
}

#[test]
fn test_panel_visible_range_scrolled() {
    let mut panel = test_panel();
    panel.view.scroll_top = LineIndex::new(50);
    // scroll_top = 50, height = 24, so visible range is 50..=73
    assert_eq!(panel.visible_range(), 50..=73);
}

#[test]
fn test_panel_visible_range_clamped_to_total() {
    let mut panel = test_panel();
    panel.view.scroll_top = LineIndex::new(990);
    // scroll_top = 990, height = 24, total = 1000
    // end would be 1013, but clamped to 999 (total_lines - 1)
    assert_eq!(panel.visible_range(), 990..=999);
}

#[test]
fn test_panel_visible_range_small_buffer() {
    let view = test_view();
    let panel = TuiPanel::new(view, 100, 24, 10);
    // total_lines = 10, so end is clamped to 9
    assert_eq!(panel.visible_range(), 0..=9);
}

#[test]
fn test_panel_scroll_to_centers() {
    let mut panel = test_panel();
    panel.scroll_to(50);
    // half_height = 12, so scroll_top should be 50 - 12 = 38
    assert_eq!(panel.view.scroll_top.as_usize(), 38);
}

#[test]
fn test_panel_scroll_to_near_top() {
    let mut panel = test_panel();
    panel.scroll_to(5);
    // half_height = 12, line 5 - 12 would underflow, so clamped to 0
    assert_eq!(panel.view.scroll_top.as_usize(), 0);
}

#[test]
fn test_panel_scroll_to_near_bottom() {
    let mut panel = test_panel();
    panel.scroll_to(995);
    // half_height = 12, so scroll_top would be 983
    // max_top = 1000 - 24 = 976, so clamped to 976
    assert_eq!(panel.view.scroll_top.as_usize(), 976);
}

#[test]
fn test_panel_cursor_position() {
    let mut panel = test_panel();
    panel.view.cursor.line = LineIndex::new(15);
    panel.view.cursor.col = ColIndex::new(8);
    assert_eq!(panel.cursor_position(), (15, 8));
}

#[test]
fn test_panel_set_cursor() {
    let mut panel = test_panel();
    panel.set_cursor(25, 10);
    assert_eq!(panel.view.cursor.line.as_usize(), 25);
    assert_eq!(panel.view.cursor.col.as_usize(), 10);
}

#[test]
fn test_panel_total_lines() {
    let panel = test_panel();
    assert_eq!(panel.total_lines(), 1000);
}

#[test]
fn test_panel_is_line_visible() {
    let panel = test_panel();
    // Default visible range is 0..=23
    assert!(panel.is_line_visible(0));
    assert!(panel.is_line_visible(23));
    assert!(!panel.is_line_visible(24));
    assert!(!panel.is_line_visible(100));
}

#[test]
fn test_panel_is_cursor_visible() {
    let mut panel = test_panel();
    panel.view.cursor.line = LineIndex::new(10);
    assert!(panel.is_cursor_visible());

    panel.view.cursor.line = LineIndex::new(50);
    assert!(!panel.is_cursor_visible());
}

#[test]
fn test_panel_visible_line_count() {
    let panel = test_panel();
    // Default visible range is 0..=23, so 24 lines
    assert_eq!(panel.visible_line_count(), 24);
}

#[test]
fn test_panel_visible_line_count_clamped() {
    let view = test_view();
    let panel = TuiPanel::new(view, 100, 24, 10);
    // Only 10 lines in buffer, so visible_line_count is 10
    assert_eq!(panel.visible_line_count(), 10);
}

#[test]
fn test_panel_ensure_cursor_visible() {
    let mut panel = test_panel();
    panel.view.cursor.line = LineIndex::new(50);
    assert!(!panel.is_cursor_visible());

    panel.ensure_cursor_visible();
    assert!(panel.is_cursor_visible());
}

#[test]
fn test_panel_view_access() {
    let panel = test_panel();
    let view = panel.view();
    assert_eq!(view.buffer_id, BufferId::from_raw(42));
}

#[test]
fn test_panel_view_mut_access() {
    let mut panel = test_panel();
    panel.view_mut().scroll_top = LineIndex::new(100);
    assert_eq!(panel.view.scroll_top.as_usize(), 100);
}

#[test]
fn test_panel_set_window_height() {
    let mut panel = test_panel();
    panel.set_window_height(40);
    assert_eq!(panel.window_height, 40);
}

#[test]
fn test_panel_set_total_lines() {
    let mut panel = test_panel();
    panel.set_total_lines(500);
    assert_eq!(panel.total_lines, 500);
}
