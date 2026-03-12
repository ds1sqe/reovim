use super::*;

struct MockPanel {
    buffer_id: u64,
    viewport_id: u64,
    visible_start: u32,
    visible_end: u32,
    cursor_line: u32,
    cursor_col: u32,
    total_lines: u32,
}

impl Panel for MockPanel {
    fn buffer_id(&self) -> u64 {
        self.buffer_id
    }

    fn viewport_id(&self) -> u64 {
        self.viewport_id
    }

    fn visible_range(&self) -> RangeInclusive<u32> {
        self.visible_start..=self.visible_end
    }

    fn scroll_to(&mut self, line: u32) {
        // Simple centering logic
        let height = self.visible_end - self.visible_start;
        let half = height / 2;
        self.visible_start = line.saturating_sub(half);
        self.visible_end = self.visible_start + height;
    }

    fn cursor_position(&self) -> (u32, u32) {
        (self.cursor_line, self.cursor_col)
    }

    fn set_cursor(&mut self, line: u32, col: u32) {
        self.cursor_line = line;
        self.cursor_col = col;
    }

    fn total_lines(&self) -> u32 {
        self.total_lines
    }
}

fn mock_panel() -> MockPanel {
    MockPanel {
        buffer_id: 1,
        viewport_id: 100,
        visible_start: 0,
        visible_end: 23,
        cursor_line: 10,
        cursor_col: 5,
        total_lines: 100,
    }
}

#[test]
fn test_panel_buffer_viewport_id() {
    let panel = mock_panel();
    assert_eq!(panel.buffer_id(), 1);
    assert_eq!(panel.viewport_id(), 100);
}

#[test]
fn test_panel_visible_range() {
    let panel = mock_panel();
    assert_eq!(panel.visible_range(), 0..=23);
}

#[test]
fn test_panel_is_line_visible() {
    let panel = mock_panel();
    assert!(panel.is_line_visible(0));
    assert!(panel.is_line_visible(23));
    assert!(!panel.is_line_visible(24));
}

#[test]
fn test_panel_cursor_position() {
    let panel = mock_panel();
    assert_eq!(panel.cursor_position(), (10, 5));
}

#[test]
fn test_panel_is_cursor_visible() {
    let panel = mock_panel();
    assert!(panel.is_cursor_visible());

    let mut panel = mock_panel();
    panel.cursor_line = 50;
    assert!(!panel.is_cursor_visible());
}

#[test]
fn test_panel_visible_line_count() {
    let panel = mock_panel();
    assert_eq!(panel.visible_line_count(), 24);
}

#[test]
fn test_panel_scroll_to() {
    let mut panel = mock_panel();
    panel.scroll_to(50);
    assert!(panel.visible_range().contains(&50));
}

#[test]
fn test_panel_ensure_cursor_visible() {
    let mut panel = mock_panel();
    panel.cursor_line = 50;
    assert!(!panel.is_cursor_visible());

    panel.ensure_cursor_visible();
    assert!(panel.is_cursor_visible());
}

#[test]
fn test_panel_ensure_cursor_visible_already_visible() {
    let mut panel = mock_panel();
    // Cursor at line 10 is already visible (range 0..=23)
    assert!(panel.is_cursor_visible());
    let old_start = panel.visible_start;
    let old_end = panel.visible_end;
    panel.ensure_cursor_visible();
    // Should not change viewport since cursor is already visible
    assert_eq!(panel.visible_start, old_start);
    assert_eq!(panel.visible_end, old_end);
}

#[test]
fn test_panel_set_cursor() {
    let mut panel = mock_panel();
    panel.set_cursor(42, 7);
    assert_eq!(panel.cursor_position(), (42, 7));
}

#[test]
fn test_panel_total_lines() {
    let panel = mock_panel();
    assert_eq!(panel.total_lines(), 100);
}

#[test]
fn test_panel_visible_line_count_after_scroll() {
    let mut panel = mock_panel();
    panel.scroll_to(50);
    // Visible line count should remain the same (24 lines)
    assert_eq!(panel.visible_line_count(), 24);
}
