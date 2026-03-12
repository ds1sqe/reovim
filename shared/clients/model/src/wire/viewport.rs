//! Viewport state types.
//!
//! A viewport represents a view into a buffer, tracking scroll position
//! and visible range. Multiple viewports can exist for the same buffer.

use serde::{Deserialize, Serialize};

/// State of a viewport (view into a buffer).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ViewportState {
    /// Viewport identifier.
    pub id: u64,
    /// Buffer being viewed.
    pub buffer_id: u64,
    /// First visible line (0-indexed).
    pub top_line: u32,
    /// Left column offset for horizontal scroll.
    pub left_col: u32,
    /// Cursor line within buffer.
    pub cursor_line: u32,
    /// Cursor column within buffer.
    pub cursor_col: u32,
}

impl ViewportState {
    /// Create a new viewport state.
    #[must_use]
    pub const fn new(id: u64, buffer_id: u64) -> Self {
        Self {
            id,
            buffer_id,
            top_line: 0,
            left_col: 0,
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    /// Set cursor position.
    #[must_use]
    pub const fn with_cursor(mut self, line: u32, col: u32) -> Self {
        self.cursor_line = line;
        self.cursor_col = col;
        self
    }

    /// Set scroll position.
    #[must_use]
    pub const fn with_scroll(mut self, top_line: u32, left_col: u32) -> Self {
        self.top_line = top_line;
        self.left_col = left_col;
        self
    }

    /// Check if a line is within the visible range.
    ///
    /// `visible_lines` is the number of lines that fit on screen.
    #[must_use]
    pub const fn is_line_visible(&self, line: u32, visible_lines: u32) -> bool {
        line >= self.top_line && line < self.top_line + visible_lines
    }
}

/// Incremental update to viewport state.
///
/// Used for efficient partial updates instead of sending full state.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ViewportUpdate {
    /// Viewport being updated.
    pub viewport_id: u64,
    /// New top line, if changed.
    pub top_line: Option<u32>,
    /// New left column, if changed.
    pub left_col: Option<u32>,
    /// New cursor line, if changed.
    pub cursor_line: Option<u32>,
    /// New cursor column, if changed.
    pub cursor_col: Option<u32>,
}

impl ViewportUpdate {
    /// Create an empty update for a viewport.
    #[must_use]
    pub const fn new(viewport_id: u64) -> Self {
        Self {
            viewport_id,
            top_line: None,
            left_col: None,
            cursor_line: None,
            cursor_col: None,
        }
    }

    /// Update the scroll position.
    #[must_use]
    pub const fn scroll_to(mut self, top_line: u32, left_col: u32) -> Self {
        self.top_line = Some(top_line);
        self.left_col = Some(left_col);
        self
    }

    /// Update the cursor position.
    #[must_use]
    pub const fn cursor_to(mut self, line: u32, col: u32) -> Self {
        self.cursor_line = Some(line);
        self.cursor_col = Some(col);
        self
    }

    /// Apply this update to a viewport state.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn apply_to(&self, state: &mut ViewportState) {
        debug_assert_eq!(self.viewport_id, state.id);
        if let Some(line) = self.top_line {
            state.top_line = line;
        }
        if let Some(col) = self.left_col {
            state.left_col = col;
        }
        if let Some(line) = self.cursor_line {
            state.cursor_line = line;
        }
        if let Some(col) = self.cursor_col {
            state.cursor_col = col;
        }
    }

    /// Check if this update is empty (no changes).
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn is_empty(&self) -> bool {
        self.top_line.is_none()
            && self.left_col.is_none()
            && self.cursor_line.is_none()
            && self.cursor_col.is_none()
    }
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
