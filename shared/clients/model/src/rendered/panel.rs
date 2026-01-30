//! Panel state types.
//!
//! A panel is a rendered view of buffer content, including scroll
//! position, visible line range, and cursor location.

use std::ops::RangeInclusive;

/// State of a rendered panel (view into a buffer).
///
/// Combines viewport state from the server with client-side
/// rendering decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelState {
    /// Viewport identifier.
    pub viewport_id: u64,
    /// Buffer being viewed.
    pub buffer_id: u64,
    /// First visible line (0-indexed).
    pub visible_start: u32,
    /// Last visible line (inclusive, 0-indexed).
    pub visible_end: u32,
    /// Cursor line within buffer.
    pub cursor_line: u32,
    /// Cursor column within buffer.
    pub cursor_col: u32,
    /// Total lines in buffer.
    pub total_lines: u32,
}

impl PanelState {
    /// Create a new panel state.
    #[must_use]
    pub const fn new(viewport_id: u64, buffer_id: u64) -> Self {
        Self {
            viewport_id,
            buffer_id,
            visible_start: 0,
            visible_end: 0,
            cursor_line: 0,
            cursor_col: 0,
            total_lines: 0,
        }
    }

    /// Set the visible range.
    #[must_use]
    pub const fn with_visible_range(mut self, start: u32, end: u32) -> Self {
        self.visible_start = start;
        self.visible_end = end;
        self
    }

    /// Set cursor position.
    #[must_use]
    pub const fn with_cursor(mut self, line: u32, col: u32) -> Self {
        self.cursor_line = line;
        self.cursor_col = col;
        self
    }

    /// Set total lines.
    #[must_use]
    pub const fn with_total_lines(mut self, total: u32) -> Self {
        self.total_lines = total;
        self
    }

    /// Get the visible line range.
    #[must_use]
    pub const fn visible_range(&self) -> (u32, u32) {
        (self.visible_start, self.visible_end)
    }

    /// Get visible range as `RangeInclusive`.
    #[must_use]
    pub const fn visible_range_inclusive(&self) -> RangeInclusive<u32> {
        self.visible_start..=self.visible_end
    }

    /// Check if a line is visible.
    #[must_use]
    pub const fn is_line_visible(&self, line: u32) -> bool {
        line >= self.visible_start && line <= self.visible_end
    }

    /// Get the number of visible lines.
    #[must_use]
    pub const fn visible_line_count(&self) -> u32 {
        if self.visible_end >= self.visible_start {
            self.visible_end - self.visible_start + 1
        } else {
            0
        }
    }

    /// Check if cursor is visible.
    #[must_use]
    pub const fn is_cursor_visible(&self) -> bool {
        self.is_line_visible(self.cursor_line)
    }

    /// Calculate scroll percentage (0.0 to 1.0).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Line counts fit well in f32 mantissa
    pub fn scroll_percentage(&self) -> f32 {
        if self.total_lines == 0 {
            return 0.0;
        }
        let max_start = self.total_lines.saturating_sub(self.visible_line_count());
        if max_start == 0 {
            return 0.0;
        }
        self.visible_start as f32 / max_start as f32
    }

    /// Check if scrolled to top.
    #[must_use]
    pub const fn is_at_top(&self) -> bool {
        self.visible_start == 0
    }

    /// Check if scrolled to bottom.
    #[must_use]
    pub const fn is_at_bottom(&self) -> bool {
        self.visible_end >= self.total_lines.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
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
    fn test_visible_range_inclusive() {
        let state = PanelState::new(1, 100).with_visible_range(10, 30);
        let range = state.visible_range_inclusive();
        assert_eq!(*range.start(), 10);
        assert_eq!(*range.end(), 30);
    }
}
