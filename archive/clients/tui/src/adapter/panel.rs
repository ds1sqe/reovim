//! Panel trait implementation for TUI.
//!
//! This module provides an adapter between the common client model's `Panel` trait
//! and TUI state. `TuiPanel` holds the viewport state directly — buffer ID, scroll
//! position, and cursor position — without depending on removed display-driver
//! layout-primitive types (`View`, `ColIndex`, `LineIndex`).

use std::ops::RangeInclusive;

use {reovim_client_model::traits::Panel, reovim_driver_display::BufferId};

/// Adapter implementing the common `Panel` trait for TUI.
///
/// Holds viewport state (buffer, scroll, cursor) and provides the methods
/// required by the `Panel` trait contract.
///
/// # ID Mapping
///
/// The common model uses `u64` for buffer/viewport IDs while TUI uses
/// kernel types (`BufferId(usize)`, `WindowId(usize)`). This adapter
/// handles the conversion transparently.
#[derive(Debug, Clone)]
pub struct TuiPanel {
    /// The buffer being viewed.
    buffer_id: BufferId,
    /// Viewport ID (window ID in TUI terms).
    viewport_id: u64,
    /// Window height in lines (for visible range calculation).
    window_height: u32,
    /// Total lines in the buffer.
    total_lines: u32,
    /// Vertical scroll offset (first visible line, 0-indexed).
    pub scroll_top: usize,
    /// Cursor line (0-indexed).
    pub cursor_line: usize,
    /// Cursor column (0-indexed).
    pub cursor_col: usize,
}

impl TuiPanel {
    /// Create a new panel adapter.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer being viewed
    /// * `viewport_id` - The viewport/window ID
    /// * `window_height` - Height of the window in lines
    /// * `total_lines` - Total number of lines in the buffer
    #[must_use]
    pub const fn new(
        buffer_id: BufferId,
        viewport_id: u64,
        window_height: u32,
        total_lines: u32,
    ) -> Self {
        Self {
            buffer_id,
            viewport_id,
            window_height,
            total_lines,
            scroll_top: 0,
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    /// Update the window height.
    pub const fn set_window_height(&mut self, height: u32) {
        self.window_height = height;
    }

    /// Update the total line count.
    pub const fn set_total_lines(&mut self, total: u32) {
        self.total_lines = total;
    }

    /// Convert `BufferId` (usize) to u64.
    #[must_use]
    const fn buffer_id_to_u64(id: BufferId) -> u64 {
        id.as_usize() as u64
    }
}

#[allow(clippy::cast_possible_truncation)]
impl Panel for TuiPanel {
    fn buffer_id(&self) -> u64 {
        Self::buffer_id_to_u64(self.buffer_id)
    }

    fn viewport_id(&self) -> u64 {
        self.viewport_id
    }

    fn visible_range(&self) -> RangeInclusive<u32> {
        let start = self.scroll_top as u32;
        // End is either scroll_top + height - 1, or total_lines - 1, whichever is smaller
        let end = (start + self.window_height)
            .saturating_sub(1)
            .min(self.total_lines.saturating_sub(1));
        start..=end
    }

    fn scroll_to(&mut self, line: u32) {
        // Center the line in the viewport
        let half_height = self.window_height / 2;
        let new_top = line.saturating_sub(half_height);
        // Clamp to valid range
        let max_top = self.total_lines.saturating_sub(self.window_height);
        let clamped_top = new_top.min(max_top);
        self.scroll_top = clamped_top as usize;
    }

    fn cursor_position(&self) -> (u32, u32) {
        (self.cursor_line as u32, self.cursor_col as u32)
    }

    fn set_cursor(&mut self, line: u32, col: u32) {
        self.cursor_line = line as usize;
        self.cursor_col = col as usize;
    }

    fn total_lines(&self) -> u32 {
        self.total_lines
    }
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
