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
/// use reovim_domain_text::TextPosition;
///
/// let pos = TextPosition::new(5, 10);
/// assert_eq!(pos.line, 5);
/// assert_eq!(pos.column, 10);
///
/// // Positions are ordered by line first, then column
/// assert!(TextPosition::new(0, 5) < TextPosition::new(1, 0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextPosition {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed, counting chars).
    pub column: usize,
}

impl TextPosition {
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

impl PartialOrd for TextPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextPosition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.line.cmp(&other.line) {
            std::cmp::Ordering::Equal => self.column.cmp(&other.column),
            ord => ord,
        }
    }
}

impl std::fmt::Display for TextPosition {
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
/// use reovim_domain_text::{Cursor, TextPosition};
///
/// let mut cursor = Cursor::new(TextPosition::new(0, 5));
///
/// // Start a selection
/// cursor.start_selection();
/// cursor.position = TextPosition::new(0, 10);
///
/// // Get normalized bounds (start before end)
/// let (start, end) = cursor.selection_bounds().unwrap();
/// assert_eq!(start, TextPosition::new(0, 5));
/// assert_eq!(end, TextPosition::new(0, 10));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    /// Current cursor position.
    pub position: TextPosition,
    /// Selection anchor (if selection is active).
    ///
    /// When `Some`, a selection extends from `anchor` to `position`.
    pub anchor: Option<TextPosition>,
    /// Preferred column for vertical movement (j/k).
    ///
    /// This preserves horizontal position when navigating through lines
    /// of varying lengths.
    pub preferred_column: Option<usize>,
}

impl Cursor {
    /// Create a new cursor at the given position.
    #[must_use]
    pub const fn new(position: TextPosition) -> Self {
        Self {
            position,
            anchor: None,
            preferred_column: None,
        }
    }

    /// Create a cursor at the origin (0, 0).
    #[must_use]
    pub const fn origin() -> Self {
        Self::new(TextPosition::origin())
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
    pub fn selection_bounds(&self) -> Option<(TextPosition, TextPosition)> {
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
