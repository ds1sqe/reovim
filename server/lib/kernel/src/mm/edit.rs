//! Edit operations for undo/redo support.
//!
//! This module defines atomic edit operations that can be recorded
//! and inverted for undo/redo functionality.

use super::Position;

/// A single atomic edit operation.
///
/// Edits are self-contained and can be inverted for undo/redo.
/// Each edit records the position where it occurred and the text involved.
///
/// # Undo/Redo
///
/// Use [`Edit::inverse`] to get the operation that undoes this edit:
/// - `Insert` becomes `Delete`
/// - `Delete` becomes `Insert`
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let insert = Edit::insert(Position::new(0, 5), "Hello");
/// assert!(insert.is_insert());
/// assert_eq!(insert.text(), "Hello");
///
/// // Get the inverse for undo
/// let undo = insert.inverse();
/// assert!(undo.is_delete());
/// assert_eq!(undo.position(), Position::new(0, 5));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Text was inserted at a position.
    Insert {
        /// Position where text was inserted.
        position: Position,
        /// The inserted text.
        text: String,
    },
    /// Text was deleted at a position.
    Delete {
        /// Position where deletion started.
        position: Position,
        /// The deleted text.
        text: String,
    },
}

impl Edit {
    /// Create an insert edit.
    #[must_use]
    pub fn insert(position: Position, text: impl Into<String>) -> Self {
        Self::Insert {
            position,
            text: text.into(),
        }
    }

    /// Create a delete edit.
    #[must_use]
    pub fn delete(position: Position, text: impl Into<String>) -> Self {
        Self::Delete {
            position,
            text: text.into(),
        }
    }

    /// Get the inverse of this edit (for undo).
    ///
    /// Insert becomes Delete and vice versa. The position and text
    /// are preserved.
    #[must_use]
    pub fn inverse(&self) -> Self {
        match self {
            Self::Insert { position, text } => Self::Delete {
                position: *position,
                text: text.clone(),
            },
            Self::Delete { position, text } => Self::Insert {
                position: *position,
                text: text.clone(),
            },
        }
    }

    /// Get the position where this edit occurred.
    #[must_use]
    pub const fn position(&self) -> Position {
        match self {
            Self::Insert { position, .. } | Self::Delete { position, .. } => *position,
        }
    }

    /// Get the text involved in this edit.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Insert { text, .. } | Self::Delete { text, .. } => text,
        }
    }

    /// Check if this edit is an insertion.
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        matches!(self, Self::Insert { .. })
    }

    /// Check if this edit is a deletion.
    #[must_use]
    pub const fn is_delete(&self) -> bool {
        matches!(self, Self::Delete { .. })
    }

    /// Check if this edit has no effect (empty text).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text().is_empty()
    }

    /// Transform this edit's position through another edit.
    ///
    /// Returns a new edit with the same text but a position adjusted
    /// to account for the effect of `against`. This is the OT inclusion
    /// transformation (IT) applied at the edit level.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// // An insert at (0,5) transformed through an earlier insert of "abc" at (0,2)
    /// let edit = Edit::insert(Position::new(0, 5), "hello");
    /// let against = Edit::insert(Position::new(0, 2), "abc");
    /// let transformed = edit.transform(&against);
    /// assert_eq!(transformed.position(), Position::new(0, 8)); // 5 + 3
    /// assert_eq!(transformed.text(), "hello"); // text unchanged
    /// ```
    #[must_use]
    pub fn transform(&self, against: &Self) -> Self {
        match self {
            Self::Insert { position, text } => Self::Insert {
                position: transform_position(*position, against),
                text: text.clone(),
            },
            Self::Delete { position, text } => Self::Delete {
                position: transform_position(*position, against),
                text: text.clone(),
            },
        }
    }
}

/// Dimensions of text for OT position transformation.
///
/// Describes the shape of a text string in terms of line count and
/// the length of the last line. Used by [`transform_position`] to
/// compute how an edit shifts positions in a 2D text buffer.
///
/// All lengths are in Unicode scalar values (chars), consistent with
/// [`Position::column`] semantics throughout reovim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDimensions {
    /// Number of newline characters in the text.
    pub line_count: usize,
    /// Character count of text after the last newline,
    /// or the full character count if there are no newlines.
    pub last_line_len: usize,
}

/// Compute the dimensions of a text string.
///
/// Returns the number of newlines and the character length of the
/// text after the last newline.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::text_dimensions;
///
/// let dims = text_dimensions("hello");
/// assert_eq!(dims.line_count, 0);
/// assert_eq!(dims.last_line_len, 5);
///
/// let dims = text_dimensions("ab\ncd\ne");
/// assert_eq!(dims.line_count, 2);
/// assert_eq!(dims.last_line_len, 1);
/// ```
#[must_use]
pub fn text_dimensions(text: &str) -> TextDimensions {
    let line_count = text.chars().filter(|&c| c == '\n').count();
    let last_line_len = text.rsplit('\n').next().map_or(0, |s| s.chars().count());
    TextDimensions {
        line_count,
        last_line_len,
    }
}

/// Compute the end position of deleted text in the pre-delete buffer state.
///
/// Given a deletion starting at `pos` with the specified `text`, returns
/// the position just past the end of the deleted region.
#[must_use]
pub fn delete_end(pos: Position, text: &str) -> Position {
    let dims = text_dimensions(text);
    if dims.line_count == 0 {
        Position::new(pos.line, pos.column + dims.last_line_len)
    } else {
        Position::new(pos.line + dims.line_count, dims.last_line_len)
    }
}

/// Transform a position through the effect of an edit.
///
/// This is the OT inclusion transformation (IT): given a position `pos`
/// in the buffer state *before* `against` was applied, returns the
/// equivalent position in the buffer state *after* `against`.
///
/// # Tie-breaking (same-position edits)
///
/// Uses left-bias convention: when `pos` equals the edit position,
/// an Insert shifts `pos` right (the insert "happened before" our position),
/// while a Delete leaves `pos` unchanged (`pos <= del_pos` → no shift).
///
/// # Character counting
///
/// All column arithmetic uses Unicode scalar values (`.chars().count()`),
/// consistent with [`Position::column`] semantics.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// // Insert "abc" at (0,2) shifts position (0,5) to (0,8)
/// let pos = transform_position(
///     Position::new(0, 5),
///     &Edit::insert(Position::new(0, 2), "abc"),
/// );
/// assert_eq!(pos, Position::new(0, 8));
/// ```
#[must_use]
pub fn transform_position(pos: Position, against: &Edit) -> Position {
    if against.is_empty() {
        return pos;
    }

    match against {
        Edit::Insert {
            position: ins_pos,
            text,
        } => transform_position_against_insert(pos, *ins_pos, text),
        Edit::Delete {
            position: del_pos,
            text,
        } => transform_position_against_delete(pos, *del_pos, text),
    }
}

/// Transform a position through an insert edit.
fn transform_position_against_insert(pos: Position, ins_pos: Position, text: &str) -> Position {
    // Position strictly before the insert is unaffected.
    if pos < ins_pos {
        return pos;
    }

    let dims = text_dimensions(text);

    if pos.line == ins_pos.line {
        // Same line as insert. Column >= ins_pos.column (due to pos >= ins_pos).
        if dims.line_count == 0 {
            // Single-line insert: shift column right.
            Position::new(pos.line, pos.column + dims.last_line_len)
        } else {
            // Multi-line insert: position moves down and column resets
            // relative to the end of the inserted text.
            Position::new(
                pos.line + dims.line_count,
                pos.column - ins_pos.column + dims.last_line_len,
            )
        }
    } else {
        // Position is on a line after the insert line: shift line down.
        Position::new(pos.line + dims.line_count, pos.column)
    }
}

/// Transform a position through a delete edit.
fn transform_position_against_delete(pos: Position, del_pos: Position, text: &str) -> Position {
    // Position at or before the delete start is unaffected.
    if pos <= del_pos {
        return pos;
    }

    let del_end = delete_end(del_pos, text);
    let dims = text_dimensions(text);

    // Position within the deleted region collapses to the delete start.
    if pos <= del_end {
        return del_pos;
    }

    if pos.line == del_end.line {
        // Position is on the last line of the deletion but after it.
        if dims.line_count == 0 {
            // Single-line delete: shift column left.
            Position::new(pos.line, pos.column - dims.last_line_len)
        } else {
            // Multi-line delete: column merges onto the delete-start line.
            Position::new(del_pos.line, del_pos.column + pos.column - del_end.column)
        }
    } else {
        // Position is on a line after the deletion: shift line up.
        Position::new(pos.line - dims.line_count, pos.column)
    }
}
