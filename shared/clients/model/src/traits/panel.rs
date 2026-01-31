//! Panel trait for buffer views.
//!
//! A panel represents a view into a buffer, with scroll position,
//! visible range, and cursor tracking.

use std::ops::RangeInclusive;

/// Trait for a panel (view into a buffer).
///
/// Panels track viewport state including scroll position, visible lines,
/// and cursor location. Platform clients implement this trait to provide
/// their specific rendering and scrolling behavior.
pub trait Panel {
    /// Get the buffer ID being viewed.
    fn buffer_id(&self) -> u64;

    /// Get the viewport ID.
    fn viewport_id(&self) -> u64;

    /// Get the range of visible lines (inclusive).
    fn visible_range(&self) -> RangeInclusive<u32>;

    /// Scroll to make the given line visible.
    ///
    /// Implementations should position the viewport so the line
    /// is visible, preferably centered or near the top.
    fn scroll_to(&mut self, line: u32);

    /// Get the cursor position (line, column).
    fn cursor_position(&self) -> (u32, u32);

    /// Set the cursor position.
    fn set_cursor(&mut self, line: u32, col: u32);

    /// Get the total number of lines in the buffer.
    fn total_lines(&self) -> u32;

    /// Check if a line is currently visible.
    fn is_line_visible(&self, line: u32) -> bool {
        self.visible_range().contains(&line)
    }

    /// Check if the cursor is currently visible.
    fn is_cursor_visible(&self) -> bool {
        let (line, _) = self.cursor_position();
        self.is_line_visible(line)
    }

    /// Get the number of visible lines.
    fn visible_line_count(&self) -> u32 {
        let range = self.visible_range();
        range.end() - range.start() + 1
    }

    /// Ensure the cursor is visible, scrolling if necessary.
    fn ensure_cursor_visible(&mut self) {
        if !self.is_cursor_visible() {
            let (line, _) = self.cursor_position();
            self.scroll_to(line);
        }
    }
}

#[cfg(test)]
mod tests {
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
}
