//! Selection state for visual mode and text operations.
//!
//! This module provides types for tracking text selection within a buffer.
//! It supports three selection modes matching vim's visual mode variants:
//! character-wise, line-wise, and block (rectangular).
//!
//! # Design Philosophy
//!
//! Following the kernel "mechanism, not policy" principle:
//! - `Selection` tracks state (anchor, mode) - mechanism
//! - Visual mode behavior is defined by modules/drivers - policy
//! - No keybinding or mode transition logic here
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! let mut selection = Selection::default();
//!
//! // Start a character-wise selection at line 5, column 10
//! selection.start(Position::new(5, 10), SelectionMode::Character);
//! assert!(selection.is_active());
//!
//! // Get bounds with cursor at line 5, column 20
//! let cursor = Position::new(5, 20);
//! let (start, end) = selection.bounds(cursor).unwrap();
//! assert_eq!(start, Position::new(5, 10));
//! assert_eq!(end, Position::new(5, 20));
//! ```

use super::Position;

/// Selection mode determining how text is selected.
///
/// These modes correspond to vim's visual mode variants:
/// - `v` for character-wise
/// - `V` for line-wise
/// - `Ctrl-V` for block (rectangular)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    /// Character-wise selection (vim's `v` mode).
    ///
    /// Selection extends character by character from anchor to cursor,
    /// spanning multiple lines if necessary.
    #[default]
    Character,

    /// Block (rectangular) selection (vim's `Ctrl-V` mode).
    ///
    /// Selection forms a rectangle defined by the anchor and cursor
    /// positions, regardless of line lengths.
    Block,

    /// Line-wise selection (vim's `V` mode).
    ///
    /// Selection extends by whole lines from the anchor line to the
    /// cursor line, inclusive.
    Line,
}

impl SelectionMode {
    /// Check if this is character-wise mode.
    #[inline]
    #[must_use]
    pub const fn is_character(&self) -> bool {
        matches!(self, Self::Character)
    }

    /// Check if this is block (rectangular) mode.
    #[inline]
    #[must_use]
    pub const fn is_block(&self) -> bool {
        matches!(self, Self::Block)
    }

    /// Check if this is line-wise mode.
    #[inline]
    #[must_use]
    pub const fn is_line(&self) -> bool {
        matches!(self, Self::Line)
    }
}

/// Selection state within a buffer.
///
/// Tracks whether a selection is active, where it started (anchor),
/// and what mode it's in. The selection extends from the anchor to
/// the current cursor position.
///
/// # Selection Direction
///
/// The anchor is where selection started. The selection can extend
/// in either direction from the anchor:
/// - Forward: cursor is after anchor
/// - Backward: cursor is before anchor
///
/// Use [`bounds()`](Self::bounds) to get normalized (start, end) positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// The position where selection started.
    pub anchor: Position,

    /// Whether selection is currently active.
    pub active: bool,

    /// The selection mode (character/block/line).
    pub mode: SelectionMode,
}

impl Selection {
    /// Create a new inactive selection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            anchor: Position::origin(),
            active: false,
            mode: SelectionMode::Character,
        }
    }

    /// Start a selection at the given position with the specified mode.
    ///
    /// This sets the anchor and activates the selection.
    ///
    /// # Arguments
    ///
    /// * `pos` - The anchor position (where selection starts)
    /// * `mode` - The selection mode
    pub const fn start(&mut self, pos: Position, mode: SelectionMode) {
        self.anchor = pos;
        self.active = true;
        self.mode = mode;
    }

    /// Start a character-wise selection at the given position.
    ///
    /// Convenience method equivalent to `start(pos, SelectionMode::Character)`.
    pub const fn start_char(&mut self, pos: Position) {
        self.start(pos, SelectionMode::Character);
    }

    /// Start a line-wise selection at the given position.
    ///
    /// Convenience method equivalent to `start(pos, SelectionMode::Line)`.
    pub const fn start_line(&mut self, pos: Position) {
        self.start(pos, SelectionMode::Line);
    }

    /// Start a block selection at the given position.
    ///
    /// Convenience method equivalent to `start(pos, SelectionMode::Block)`.
    pub const fn start_block(&mut self, pos: Position) {
        self.start(pos, SelectionMode::Block);
    }

    /// Clear (deactivate) the selection.
    ///
    /// The anchor position is preserved but the selection becomes inactive.
    pub const fn clear(&mut self) {
        self.active = false;
    }

    /// Check if selection is currently active.
    #[inline]
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Get the selection mode.
    #[inline]
    #[must_use]
    pub const fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Change the selection mode without changing anchor or active state.
    pub const fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
    }

    /// Get selection bounds in document order (start <= end).
    ///
    /// Returns `None` if selection is not active.
    /// The returned positions are always ordered so that start comes
    /// before or equals end in document order.
    ///
    /// # Arguments
    ///
    /// * `cursor_pos` - The current cursor position (selection endpoint)
    #[must_use]
    pub fn bounds(&self, cursor_pos: Position) -> Option<(Position, Position)> {
        if !self.active {
            return None;
        }

        if self.anchor <= cursor_pos {
            Some((self.anchor, cursor_pos))
        } else {
            Some((cursor_pos, self.anchor))
        }
    }

    /// Get block selection bounds (top-left, bottom-right).
    ///
    /// For block selections, this returns the rectangle corners:
    /// - Top-left: (`min_line`, `min_column`)
    /// - Bottom-right: (`max_line`, `max_column`)
    ///
    /// Returns `None` if selection is not active or not in block mode.
    ///
    /// # Arguments
    ///
    /// * `cursor_pos` - The current cursor position
    #[must_use]
    pub fn block_bounds(&self, cursor_pos: Position) -> Option<(Position, Position)> {
        if !self.active || self.mode != SelectionMode::Block {
            return None;
        }

        let min_line = self.anchor.line.min(cursor_pos.line);
        let max_line = self.anchor.line.max(cursor_pos.line);
        let min_col = self.anchor.column.min(cursor_pos.column);
        let max_col = self.anchor.column.max(cursor_pos.column);

        Some((Position::new(min_line, min_col), Position::new(max_line, max_col)))
    }

    /// Get line-wise selection bounds (`start_line`, `end_line`).
    ///
    /// For line selections, this returns the range of selected lines.
    /// Returns `None` if selection is not active.
    ///
    /// # Arguments
    ///
    /// * `cursor_pos` - The current cursor position
    #[must_use]
    pub fn line_bounds(&self, cursor_pos: Position) -> Option<(usize, usize)> {
        if !self.active {
            return None;
        }

        let start_line = self.anchor.line.min(cursor_pos.line);
        let end_line = self.anchor.line.max(cursor_pos.line);

        Some((start_line, end_line))
    }

    /// Check if a position is within the selection.
    ///
    /// The behavior depends on the selection mode:
    /// - Character: position is between start and end
    /// - Block: position is within the rectangle
    /// - Line: position's line is within the line range
    ///
    /// # Arguments
    ///
    /// * `pos` - The position to check
    /// * `cursor_pos` - The current cursor position (selection endpoint)
    #[must_use]
    pub fn contains(&self, pos: Position, cursor_pos: Position) -> bool {
        if !self.active {
            return false;
        }

        match self.mode {
            SelectionMode::Character => {
                if let Some((start, end)) = self.bounds(cursor_pos) {
                    pos >= start && pos <= end
                } else {
                    false
                }
            }
            SelectionMode::Block => {
                if let Some((top_left, bottom_right)) = self.block_bounds(cursor_pos) {
                    pos.line >= top_left.line
                        && pos.line <= bottom_right.line
                        && pos.column >= top_left.column
                        && pos.column <= bottom_right.column
                } else {
                    false
                }
            }
            SelectionMode::Line => {
                if let Some((start_line, end_line)) = self.line_bounds(cursor_pos) {
                    pos.line >= start_line && pos.line <= end_line
                } else {
                    false
                }
            }
        }
    }

    /// Get the number of lines in the selection.
    ///
    /// Returns 0 if selection is not active.
    ///
    /// # Arguments
    ///
    /// * `cursor_pos` - The current cursor position
    #[must_use]
    pub fn line_count(&self, cursor_pos: Position) -> usize {
        if !self.active {
            return 0;
        }

        let start_line = self.anchor.line.min(cursor_pos.line);
        let end_line = self.anchor.line.max(cursor_pos.line);
        end_line - start_line + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_mode_default() {
        let mode = SelectionMode::default();
        assert!(mode.is_character());
        assert!(!mode.is_block());
        assert!(!mode.is_line());
    }

    #[test]
    fn test_selection_mode_checks() {
        assert!(SelectionMode::Character.is_character());
        assert!(SelectionMode::Block.is_block());
        assert!(SelectionMode::Line.is_line());
    }

    #[test]
    fn test_selection_new() {
        let sel = Selection::new();
        assert!(!sel.is_active());
        assert_eq!(sel.anchor, Position::origin());
        assert_eq!(sel.mode, SelectionMode::Character);
    }

    #[test]
    fn test_selection_start() {
        let mut sel = Selection::new();
        sel.start(Position::new(5, 10), SelectionMode::Block);

        assert!(sel.is_active());
        assert_eq!(sel.anchor, Position::new(5, 10));
        assert_eq!(sel.mode, SelectionMode::Block);
    }

    #[test]
    fn test_selection_clear() {
        let mut sel = Selection::new();
        sel.start(Position::new(5, 10), SelectionMode::Character);
        assert!(sel.is_active());

        sel.clear();
        assert!(!sel.is_active());
        // Anchor is preserved
        assert_eq!(sel.anchor, Position::new(5, 10));
    }

    #[test]
    fn test_selection_bounds_forward() {
        let mut sel = Selection::new();
        sel.start(Position::new(1, 5), SelectionMode::Character);

        let cursor = Position::new(3, 10);
        let (start, end) = sel.bounds(cursor).unwrap();

        assert_eq!(start, Position::new(1, 5));
        assert_eq!(end, Position::new(3, 10));
    }

    #[test]
    fn test_selection_bounds_backward() {
        let mut sel = Selection::new();
        sel.start(Position::new(5, 15), SelectionMode::Character);

        let cursor = Position::new(2, 3);
        let (start, end) = sel.bounds(cursor).unwrap();

        // Should be normalized
        assert_eq!(start, Position::new(2, 3));
        assert_eq!(end, Position::new(5, 15));
    }

    #[test]
    fn test_selection_bounds_when_inactive() {
        let sel = Selection::new();
        assert!(sel.bounds(Position::new(0, 0)).is_none());
    }

    #[test]
    fn test_selection_block_bounds() {
        let mut sel = Selection::new();
        sel.start(Position::new(5, 20), SelectionMode::Block);

        let cursor = Position::new(2, 5);
        let (top_left, bottom_right) = sel.block_bounds(cursor).unwrap();

        assert_eq!(top_left, Position::new(2, 5));
        assert_eq!(bottom_right, Position::new(5, 20));
    }

    #[test]
    fn test_selection_block_bounds_non_block_mode() {
        let mut sel = Selection::new();
        sel.start(Position::new(5, 20), SelectionMode::Character);

        let cursor = Position::new(2, 5);
        assert!(sel.block_bounds(cursor).is_none());
    }

    #[test]
    fn test_selection_line_bounds() {
        let mut sel = Selection::new();
        sel.start(Position::new(10, 5), SelectionMode::Line);

        let cursor = Position::new(5, 0);
        let (start_line, end_line) = sel.line_bounds(cursor).unwrap();

        assert_eq!(start_line, 5);
        assert_eq!(end_line, 10);
    }

    #[test]
    fn test_selection_mode_switching() {
        let mut sel = Selection::new();
        sel.start(Position::new(0, 0), SelectionMode::Character);
        assert!(sel.mode().is_character());

        sel.set_mode(SelectionMode::Block);
        assert!(sel.mode().is_block());

        sel.set_mode(SelectionMode::Line);
        assert!(sel.mode().is_line());
    }

    #[test]
    fn test_selection_contains_character() {
        let mut sel = Selection::new();
        sel.start(Position::new(1, 0), SelectionMode::Character);
        let cursor = Position::new(3, 10);

        assert!(sel.contains(Position::new(2, 5), cursor));
        assert!(sel.contains(Position::new(1, 0), cursor)); // Start
        assert!(sel.contains(Position::new(3, 10), cursor)); // End
        assert!(!sel.contains(Position::new(0, 0), cursor)); // Before
        assert!(!sel.contains(Position::new(4, 0), cursor)); // After
    }

    #[test]
    fn test_selection_contains_block() {
        let mut sel = Selection::new();
        sel.start(Position::new(1, 5), SelectionMode::Block);
        let cursor = Position::new(3, 15);

        assert!(sel.contains(Position::new(2, 10), cursor)); // Inside
        assert!(sel.contains(Position::new(1, 5), cursor)); // Corner
        assert!(!sel.contains(Position::new(2, 3), cursor)); // Left of block
        assert!(!sel.contains(Position::new(2, 20), cursor)); // Right of block
    }

    #[test]
    fn test_selection_contains_line() {
        let mut sel = Selection::new();
        sel.start(Position::new(2, 0), SelectionMode::Line);
        let cursor = Position::new(5, 10);

        assert!(sel.contains(Position::new(3, 0), cursor)); // Inside
        assert!(sel.contains(Position::new(3, 100), cursor)); // Inside, any column
        assert!(!sel.contains(Position::new(1, 0), cursor)); // Before
        assert!(!sel.contains(Position::new(6, 0), cursor)); // After
    }

    #[test]
    fn test_selection_line_count() {
        let mut sel = Selection::new();
        sel.start(Position::new(3, 0), SelectionMode::Character);

        assert_eq!(sel.line_count(Position::new(3, 10)), 1); // Same line
        assert_eq!(sel.line_count(Position::new(5, 0)), 3); // Lines 3, 4, 5
        assert_eq!(sel.line_count(Position::new(1, 0)), 3); // Lines 1, 2, 3 (backward)
    }

    #[test]
    fn test_selection_line_count_inactive() {
        let sel = Selection::new();
        assert_eq!(sel.line_count(Position::new(5, 0)), 0);
    }

    #[test]
    fn test_selection_convenience_methods() {
        let mut sel = Selection::new();

        sel.start_char(Position::new(0, 0));
        assert!(sel.mode().is_character());

        sel.start_line(Position::new(1, 0));
        assert!(sel.mode().is_line());

        sel.start_block(Position::new(2, 0));
        assert!(sel.mode().is_block());
    }
}
