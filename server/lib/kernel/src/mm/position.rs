//! Position and cursor types for text navigation.
//!
//! This module provides the fundamental types for representing locations
//! within a text buffer and tracking cursor state.

/// A position within a text buffer.
///
/// Positions are 0-indexed for both line and column.
/// Column counts Unicode scalar values (chars), not bytes or graphemes.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let pos = Position::new(5, 10);
/// assert_eq!(pos.line, 5);
/// assert_eq!(pos.column, 10);
///
/// // Positions are ordered by line first, then column
/// assert!(Position::new(0, 5) < Position::new(1, 0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Position {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed, counting chars).
    pub column: usize,
}

impl Position {
    /// Create a new position.
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Create a position at the start of the buffer (0, 0).
    #[must_use]
    pub const fn origin() -> Self {
        Self { line: 0, column: 0 }
    }

    /// Create a position at the start of a specific line.
    #[must_use]
    pub const fn line_start(line: usize) -> Self {
        Self { line, column: 0 }
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.line.cmp(&other.line) {
            std::cmp::Ordering::Equal => self.column.cmp(&other.column),
            ord => ord,
        }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.column + 1)
    }
}

/// Cursor state within a buffer.
///
/// The cursor tracks the current position, an optional selection anchor,
/// and a preferred column for vertical movement.
///
/// # Selection
///
/// When `anchor` is `Some`, a selection extends from `anchor` to `position`.
/// The selection can be in either direction (anchor before or after position).
///
/// # Preferred Column
///
/// When moving vertically through lines of varying lengths, the cursor
/// attempts to maintain its horizontal position. The `preferred_column`
/// stores this target column.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let mut cursor = Cursor::new(Position::new(0, 5));
///
/// // Start a selection
/// cursor.start_selection();
/// cursor.position = Position::new(0, 10);
///
/// // Get normalized bounds (start before end)
/// let (start, end) = cursor.selection_bounds().unwrap();
/// assert_eq!(start, Position::new(0, 5));
/// assert_eq!(end, Position::new(0, 10));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Current cursor position.
    pub position: Position,
    /// Selection anchor (if selection is active).
    ///
    /// When `Some`, a selection extends from `anchor` to `position`.
    pub anchor: Option<Position>,
    /// Preferred column for vertical movement (j/k).
    ///
    /// This preserves horizontal position when navigating through lines
    /// of varying lengths.
    pub preferred_column: Option<usize>,
}

impl Cursor {
    /// Create a new cursor at the given position.
    #[must_use]
    pub const fn new(position: Position) -> Self {
        Self {
            position,
            anchor: None,
            preferred_column: None,
        }
    }

    /// Create a cursor at the origin (0, 0).
    #[must_use]
    pub const fn origin() -> Self {
        Self::new(Position::origin())
    }

    /// Start a selection at the current position.
    pub const fn start_selection(&mut self) {
        self.anchor = Some(self.position);
    }

    /// Clear the current selection.
    pub const fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Check if a selection is active.
    #[must_use]
    pub const fn has_selection(&self) -> bool {
        self.anchor.is_some()
    }

    /// Get the selection bounds (start, end) in document order.
    ///
    /// Returns `None` if no selection is active.
    /// The returned positions are always ordered (start <= end).
    #[must_use]
    pub fn selection_bounds(&self) -> Option<(Position, Position)> {
        self.anchor.map(|anchor| {
            if anchor <= self.position {
                (anchor, self.position)
            } else {
                (self.position, anchor)
            }
        })
    }

    /// Set preferred column to current column.
    ///
    /// Call this after horizontal movements to update the target column
    /// for subsequent vertical movements.
    pub const fn update_preferred_column(&mut self) {
        self.preferred_column = Some(self.position.column);
    }

    /// Clear preferred column.
    ///
    /// Call this after horizontal movements that should reset
    /// the vertical movement target.
    pub const fn clear_preferred_column(&mut self) {
        self.preferred_column = None;
    }

    /// Get the effective column for vertical movement.
    ///
    /// Returns the preferred column if set, otherwise the current column.
    #[must_use]
    pub fn effective_column(&self) -> usize {
        self.preferred_column.unwrap_or(self.position.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Position Tests ===

    #[test]
    fn position_new() {
        let pos = Position::new(3, 7);
        assert_eq!(pos.line, 3);
        assert_eq!(pos.column, 7);
    }

    #[test]
    fn position_line_start() {
        let pos = Position::line_start(5);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn position_line_start_zero() {
        let pos = Position::line_start(0);
        assert_eq!(pos, Position::origin());
    }

    #[test]
    fn position_default() {
        let pos = Position::default();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
        assert_eq!(pos, Position::origin());
    }

    #[test]
    fn position_eq() {
        assert_eq!(Position::new(1, 2), Position::new(1, 2));
        assert_ne!(Position::new(1, 2), Position::new(1, 3));
        assert_ne!(Position::new(1, 2), Position::new(2, 2));
    }

    #[test]
    fn position_ord_same_line() {
        assert!(Position::new(0, 0) < Position::new(0, 1));
        assert!(Position::new(0, 5) < Position::new(0, 10));
        assert!(Position::new(0, 10) > Position::new(0, 5));
    }

    #[test]
    fn position_ord_different_lines() {
        assert!(Position::new(0, 100) < Position::new(1, 0));
        assert!(Position::new(1, 0) < Position::new(2, 0));
        assert!(Position::new(3, 0) > Position::new(2, 99));
    }

    #[test]
    fn position_ord_equal() {
        let a = Position::new(5, 5);
        let b = Position::new(5, 5);
        assert!(a <= b);
        assert!(a >= b);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn position_partial_ord_consistent_with_ord() {
        let a = Position::new(1, 5);
        let b = Position::new(2, 3);
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
        assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn position_display() {
        let pos = Position::new(0, 0);
        assert_eq!(format!("{pos}"), "1:1");

        let pos = Position::new(9, 19);
        assert_eq!(format!("{pos}"), "10:20");
    }

    #[test]
    fn position_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Position::new(0, 0));
        set.insert(Position::new(1, 0));
        set.insert(Position::new(0, 0)); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn position_clone_and_copy() {
        let a = Position::new(3, 4);
        let b = a; // copy
        assert_eq!(a, b);
    }

    // === Cursor Tests ===

    #[test]
    fn cursor_new() {
        let cursor = Cursor::new(Position::new(5, 10));
        assert_eq!(cursor.position, Position::new(5, 10));
        assert!(cursor.anchor.is_none());
        assert!(cursor.preferred_column.is_none());
    }

    #[test]
    fn cursor_origin() {
        let cursor = Cursor::origin();
        assert_eq!(cursor.position, Position::origin());
        assert!(!cursor.has_selection());
    }

    #[test]
    fn cursor_default() {
        let cursor = Cursor::default();
        assert_eq!(cursor.position, Position::origin());
        assert!(cursor.anchor.is_none());
        assert!(cursor.preferred_column.is_none());
    }

    #[test]
    fn cursor_start_and_clear_selection() {
        let mut cursor = Cursor::new(Position::new(2, 5));
        assert!(!cursor.has_selection());
        assert!(cursor.selection_bounds().is_none());

        cursor.start_selection();
        assert!(cursor.has_selection());
        assert_eq!(cursor.anchor, Some(Position::new(2, 5)));

        cursor.clear_selection();
        assert!(!cursor.has_selection());
        assert!(cursor.anchor.is_none());
    }

    #[test]
    fn cursor_selection_bounds_forward() {
        let mut cursor = Cursor::new(Position::new(0, 0));
        cursor.start_selection();
        cursor.position = Position::new(2, 10);

        let (start, end) = cursor.selection_bounds().unwrap();
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(2, 10));
    }

    #[test]
    fn cursor_selection_bounds_backward() {
        let mut cursor = Cursor::new(Position::new(5, 15));
        cursor.start_selection();
        cursor.position = Position::new(1, 3);

        let (start, end) = cursor.selection_bounds().unwrap();
        assert_eq!(start, Position::new(1, 3));
        assert_eq!(end, Position::new(5, 15));
    }

    #[test]
    fn cursor_selection_bounds_same_position() {
        let mut cursor = Cursor::new(Position::new(3, 3));
        cursor.start_selection();
        // cursor.position stays at anchor

        let (start, end) = cursor.selection_bounds().unwrap();
        assert_eq!(start, Position::new(3, 3));
        assert_eq!(end, Position::new(3, 3));
    }

    #[test]
    fn cursor_selection_bounds_no_selection() {
        let cursor = Cursor::new(Position::new(0, 0));
        assert!(cursor.selection_bounds().is_none());
    }

    #[test]
    fn cursor_preferred_column_lifecycle() {
        let mut cursor = Cursor::new(Position::new(0, 15));
        assert!(cursor.preferred_column.is_none());
        assert_eq!(cursor.effective_column(), 15);

        cursor.update_preferred_column();
        assert_eq!(cursor.preferred_column, Some(15));
        assert_eq!(cursor.effective_column(), 15);

        // Move to shorter line
        cursor.position = Position::new(1, 5);
        // preferred_column still 15
        assert_eq!(cursor.effective_column(), 15);

        cursor.clear_preferred_column();
        assert!(cursor.preferred_column.is_none());
        assert_eq!(cursor.effective_column(), 5);
    }

    #[test]
    fn cursor_effective_column_without_preferred() {
        let cursor = Cursor::new(Position::new(0, 42));
        assert_eq!(cursor.effective_column(), 42);
    }

    #[test]
    fn cursor_eq() {
        let a = Cursor::new(Position::new(1, 2));
        let b = Cursor::new(Position::new(1, 2));
        let c = Cursor::new(Position::new(1, 3));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn cursor_clone() {
        let mut cursor = Cursor::new(Position::new(1, 2));
        cursor.start_selection();
        cursor.update_preferred_column();

        let cloned = cursor;
        assert_eq!(cursor.position, cloned.position);
        assert_eq!(cursor.anchor, cloned.anchor);
        assert_eq!(cursor.preferred_column, cloned.preferred_column);
    }
}
