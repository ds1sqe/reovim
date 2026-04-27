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
#[path = "panel_tests.rs"]
mod tests;
