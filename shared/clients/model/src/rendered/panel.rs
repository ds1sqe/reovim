//! Panel state types.
//!
//! A panel is a rendered view of buffer content, including scroll
//! position, visible line range, and cursor location.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

/// State of a rendered panel (view into a buffer).
///
/// Combines viewport state from the server with client-side
/// rendering decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
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
#[path = "panel_tests.rs"]
mod tests;
