//! Viewport and scrolling management for windows
//!
//! This module handles viewport positioning and scroll behavior, ensuring
//! the cursor remains visible within the window bounds.

use crate::buffer::Buffer;

use super::{Anchor, Window};

impl Window {
    /// Get buffer anchor (scroll position) from content source
    #[must_use]
    pub const fn buffer_anchor(&self) -> Option<Anchor> {
        match &self.source {
            super::WindowContentSource::FileBuffer { buffer_anchor, .. }
            | super::WindowContentSource::PluginBuffer { buffer_anchor, .. } => {
                Some(*buffer_anchor)
            }
        }
    }

    /// Set buffer anchor in content source (if applicable)
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_buffer_anchor(&mut self, new_anchor: Anchor) {
        match &mut self.source {
            super::WindowContentSource::FileBuffer { buffer_anchor, .. }
            | super::WindowContentSource::PluginBuffer { buffer_anchor, .. } => {
                *buffer_anchor = new_anchor;
            }
        }
    }

    /// Get effective cursor Y position (active buffer cursor or window-saved cursor)
    #[must_use]
    pub const fn effective_cursor_y(&self, buf: &Buffer) -> u16 {
        if self.is_active {
            buf.cur.y
        } else {
            self.cursor.y
        }
    }

    /// Update viewport scroll position to keep cursor visible
    ///
    /// Returns `true` if scrolling occurred, `false` if cursor was already visible.
    pub fn update_scroll(&mut self, cursor_y: u16) -> bool {
        let visible_height = self.height;
        let Some(mut buffer_anchor) = self.buffer_anchor() else {
            return false;
        };
        let scroll_offset = buffer_anchor.y;

        // Scroll up if cursor is above visible area
        if cursor_y < scroll_offset {
            buffer_anchor.y = cursor_y;
            self.set_buffer_anchor(buffer_anchor);
            true
        }
        // Scroll down if cursor is below visible area
        else if cursor_y >= scroll_offset + visible_height {
            buffer_anchor.y = cursor_y.saturating_sub(visible_height) + 1;
            self.set_buffer_anchor(buffer_anchor);
            true
        } else {
            false
        }
    }

    /// Get the viewport bounds (`top_line`, `bottom_line`) for this window
    ///
    /// Returns (`top_line`, `bottom_line`) where both are 0-indexed buffer line numbers.
    /// `bottom_line` is the last visible line (inclusive).
    #[must_use]
    pub fn viewport_bounds(&self) -> (u32, u32) {
        let top_line = self.buffer_anchor().map_or(0, |a| u32::from(a.y));
        let bottom_line = top_line + u32::from(self.height).saturating_sub(1);
        (top_line, bottom_line)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::{SignColumnMode, WindowContentSource},
        *,
    };

    fn create_test_window(height: u16) -> Window {
        Window {
            id: 0,
            source: WindowContentSource::FileBuffer {
                buffer_id: 0,
                buffer_anchor: Anchor { x: 0, y: 0 },
            },
            anchor: Anchor { x: 0, y: 0 },
            width: 80,
            height,
            z_order: 0,
            is_active: true,
            is_floating: false,
            line_number: Some(super::super::LineNumber::default()),
            scrollbar_enabled: false,
            sign_column_mode: SignColumnMode::Auto,
            cursor: super::super::Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
        }
    }

    #[test]
    fn test_update_scroll_cursor_in_view() {
        let mut win = create_test_window(10);
        win.update_scroll(5); // cursor at line 5, viewport 0-9
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 0); // no scroll needed
    }

    #[test]
    fn test_update_scroll_cursor_below_viewport() {
        let mut win = create_test_window(10);
        win.update_scroll(15); // cursor at line 15, viewport 0-9
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 6); // scroll to show cursor at bottom
    }

    #[test]
    fn test_update_scroll_cursor_above_viewport() {
        let mut win = create_test_window(10);
        win.set_buffer_anchor(Anchor { x: 0, y: 20 }); // viewport starts at line 20
        win.update_scroll(5); // cursor at line 5
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 5); // scroll up to cursor
    }

    #[test]
    fn test_update_scroll_cursor_at_viewport_edge() {
        let mut win = create_test_window(10);
        win.update_scroll(9); // cursor at last visible line
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 0); // still in view

        win.update_scroll(10); // cursor just below viewport
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 1); // scroll by 1
    }
}
