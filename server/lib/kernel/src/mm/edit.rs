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

#[cfg(test)]
mod tests {
    use super::*;

    // === Edit construction ===

    #[test]
    fn insert_edit_fields() {
        let edit = Edit::insert(Position::new(3, 7), "hello");
        assert!(edit.is_insert());
        assert!(!edit.is_delete());
        assert_eq!(edit.position(), Position::new(3, 7));
        assert_eq!(edit.text(), "hello");
    }

    #[test]
    fn delete_edit_fields() {
        let edit = Edit::delete(Position::new(1, 0), "removed");
        assert!(edit.is_delete());
        assert!(!edit.is_insert());
        assert_eq!(edit.position(), Position::new(1, 0));
        assert_eq!(edit.text(), "removed");
    }

    #[test]
    fn insert_with_string_type() {
        let edit = Edit::insert(Position::origin(), String::from("owned"));
        assert_eq!(edit.text(), "owned");
    }

    #[test]
    fn delete_with_string_type() {
        let edit = Edit::delete(Position::origin(), String::from("owned"));
        assert_eq!(edit.text(), "owned");
    }

    // === is_empty ===

    #[test]
    fn empty_insert() {
        let edit = Edit::insert(Position::origin(), "");
        assert!(edit.is_empty());
    }

    #[test]
    fn empty_delete() {
        let edit = Edit::delete(Position::origin(), "");
        assert!(edit.is_empty());
    }

    #[test]
    fn non_empty_insert() {
        let edit = Edit::insert(Position::origin(), "x");
        assert!(!edit.is_empty());
    }

    #[test]
    fn non_empty_delete() {
        let edit = Edit::delete(Position::origin(), "y");
        assert!(!edit.is_empty());
    }

    // === inverse ===

    #[test]
    fn inverse_insert_becomes_delete() {
        let edit = Edit::insert(Position::new(2, 3), "text");
        let inv = edit.inverse();
        assert!(inv.is_delete());
        assert_eq!(inv.position(), Position::new(2, 3));
        assert_eq!(inv.text(), "text");
    }

    #[test]
    fn inverse_delete_becomes_insert() {
        let edit = Edit::delete(Position::new(5, 0), "gone");
        let inv = edit.inverse();
        assert!(inv.is_insert());
        assert_eq!(inv.position(), Position::new(5, 0));
        assert_eq!(inv.text(), "gone");
    }

    #[test]
    fn double_inverse_is_identity() {
        let edit = Edit::insert(Position::new(1, 1), "abc");
        let double = edit.inverse().inverse();
        assert_eq!(double, edit);
    }

    // === Clone and Eq ===

    #[test]
    fn edit_clone_eq() {
        let edit = Edit::insert(Position::new(0, 5), "clone");
        let cloned = edit.clone();
        assert_eq!(edit, cloned);
    }

    #[test]
    fn edit_ne() {
        let a = Edit::insert(Position::new(0, 0), "a");
        let b = Edit::insert(Position::new(0, 0), "b");
        assert_ne!(a, b);

        let c = Edit::insert(Position::new(0, 0), "a");
        let d = Edit::delete(Position::new(0, 0), "a");
        assert_ne!(c, d);
    }

    #[test]
    fn edit_debug_format() {
        let edit = Edit::insert(Position::origin(), "hi");
        let debug = format!("{edit:?}");
        assert!(debug.contains("Insert"));
        assert!(debug.contains("hi"));
    }

    // === TextDimensions ===

    #[test]
    fn text_dimensions_single_char() {
        let dims = text_dimensions("x");
        assert_eq!(dims.line_count, 0);
        assert_eq!(dims.last_line_len, 1);
    }

    #[test]
    fn text_dimensions_two_lines() {
        let dims = text_dimensions("ab\ncd");
        assert_eq!(dims.line_count, 1);
        assert_eq!(dims.last_line_len, 2);
    }

    #[test]
    fn text_dimensions_three_lines() {
        let dims = text_dimensions("a\nb\nc");
        assert_eq!(dims.line_count, 2);
        assert_eq!(dims.last_line_len, 1);
    }

    #[test]
    fn text_dimensions_only_newlines() {
        let dims = text_dimensions("\n\n");
        assert_eq!(dims.line_count, 2);
        assert_eq!(dims.last_line_len, 0);
    }

    #[test]
    fn text_dimensions_unicode() {
        // "ab\ncd" with unicode: chars not bytes
        let dims = text_dimensions("ab\ncde");
        assert_eq!(dims.line_count, 1);
        assert_eq!(dims.last_line_len, 3);
    }

    #[test]
    fn text_dimensions_struct_eq() {
        let a = TextDimensions {
            line_count: 1,
            last_line_len: 5,
        };
        let b = TextDimensions {
            line_count: 1,
            last_line_len: 5,
        };
        assert_eq!(a, b);

        let c = TextDimensions {
            line_count: 2,
            last_line_len: 5,
        };
        assert_ne!(a, c);
    }

    #[test]
    fn text_dimensions_debug() {
        let dims = text_dimensions("test");
        let debug = format!("{dims:?}");
        assert!(debug.contains("TextDimensions"));
    }

    // === delete_end ===

    #[test]
    fn delete_end_single_line() {
        let end = delete_end(Position::new(0, 3), "abc");
        assert_eq!(end, Position::new(0, 6));
    }

    #[test]
    fn delete_end_multi_line() {
        let end = delete_end(Position::new(0, 3), "hello\nworld");
        assert_eq!(end, Position::new(1, 5));
    }

    #[test]
    fn delete_end_trailing_newline() {
        let end = delete_end(Position::new(2, 0), "hello\n");
        assert_eq!(end, Position::new(3, 0));
    }

    #[test]
    fn delete_end_empty_text() {
        let end = delete_end(Position::new(5, 3), "");
        assert_eq!(end, Position::new(5, 3));
    }

    #[test]
    fn delete_end_just_newline() {
        let end = delete_end(Position::new(0, 0), "\n");
        assert_eq!(end, Position::new(1, 0));
    }

    // === transform_position ===

    // -- Empty edit is no-op --

    #[test]
    fn transform_empty_insert_noop() {
        let pos = Position::new(3, 7);
        let result = transform_position(pos, &Edit::insert(Position::new(0, 0), ""));
        assert_eq!(result, pos);
    }

    #[test]
    fn transform_empty_delete_noop() {
        let pos = Position::new(3, 7);
        let result = transform_position(pos, &Edit::delete(Position::new(0, 0), ""));
        assert_eq!(result, pos);
    }

    // -- Insert: position before insert is unaffected --

    #[test]
    fn transform_pos_before_insert_same_line() {
        let result =
            transform_position(Position::new(0, 2), &Edit::insert(Position::new(0, 10), "abc"));
        assert_eq!(result, Position::new(0, 2));
    }

    #[test]
    fn transform_pos_before_insert_earlier_line() {
        let result =
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(3, 0), "text"));
        assert_eq!(result, Position::new(0, 5));
    }

    // -- Insert: single-line same line shifts column --

    #[test]
    fn transform_pos_after_single_line_insert() {
        let result =
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 2), "abc"));
        assert_eq!(result, Position::new(0, 8)); // 5 + 3
    }

    // -- Insert: at same position (tie-breaking: left-bias) --

    #[test]
    fn transform_pos_at_insert_position() {
        let result =
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 5), "xyz"));
        assert_eq!(result, Position::new(0, 8)); // shifted right
    }

    // -- Insert: multi-line same line --

    #[test]
    fn transform_pos_after_multiline_insert_same_line() {
        // Insert "ab\nc" at (0,2), position (0,5) -> (1, 5-2+1 = 4)
        let result =
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 2), "ab\nc"));
        assert_eq!(result, Position::new(1, 4));
    }

    // -- Insert: different line after insert shifts line down --

    #[test]
    fn transform_pos_later_line_after_insert() {
        // Insert 2 newlines at line 1, position at line 5 shifts to line 7
        let result =
            transform_position(Position::new(5, 3), &Edit::insert(Position::new(1, 0), "a\nb\n"));
        assert_eq!(result, Position::new(7, 3)); // 5 + 2
    }

    // -- Delete: position before delete is unaffected --

    #[test]
    fn transform_pos_before_delete() {
        let result =
            transform_position(Position::new(0, 1), &Edit::delete(Position::new(0, 5), "abc"));
        assert_eq!(result, Position::new(0, 1));
    }

    // -- Delete: position at delete start is unaffected (tie-breaking) --

    #[test]
    fn transform_pos_at_delete_start() {
        let result =
            transform_position(Position::new(0, 5), &Edit::delete(Position::new(0, 5), "abc"));
        assert_eq!(result, Position::new(0, 5));
    }

    // -- Delete: position within deleted region collapses --

    #[test]
    fn transform_pos_within_delete_single_line() {
        let result =
            transform_position(Position::new(0, 4), &Edit::delete(Position::new(0, 2), "abcde"));
        assert_eq!(result, Position::new(0, 2));
    }

    #[test]
    fn transform_pos_within_delete_multiline() {
        let result = transform_position(
            Position::new(1, 2),
            &Edit::delete(Position::new(0, 3), "hello\nworld"),
        );
        assert_eq!(result, Position::new(0, 3));
    }

    // -- Delete: position after single-line delete shifts left --

    #[test]
    fn transform_pos_after_single_line_delete() {
        let result =
            transform_position(Position::new(0, 8), &Edit::delete(Position::new(0, 2), "abc"));
        assert_eq!(result, Position::new(0, 5)); // 8 - 3
    }

    // -- Delete: position on last line of multiline delete, after delete end --

    #[test]
    fn transform_pos_after_multiline_delete_same_end_line() {
        // Delete "hello\nworld" at (0,3), del_end = (1,5)
        // Position (1,8) -> (0, 3 + 8 - 5) = (0, 6)
        let result = transform_position(
            Position::new(1, 8),
            &Edit::delete(Position::new(0, 3), "hello\nworld"),
        );
        assert_eq!(result, Position::new(0, 6));
    }

    // -- Delete: position on later line shifts line up --

    #[test]
    fn transform_pos_later_line_after_delete() {
        // Delete 2 lines at (1,0), position (5,3) shifts to (3,3)
        let result = transform_position(
            Position::new(5, 3),
            &Edit::delete(Position::new(1, 0), "hello\nworld\n"),
        );
        assert_eq!(result, Position::new(3, 3)); // 5 - 2
    }

    // === Edit::transform ===

    #[test]
    fn transform_edit_insert_preserves_text() {
        let edit = Edit::insert(Position::new(0, 5), "hello");
        let against = Edit::insert(Position::new(0, 2), "abc");
        let transformed = edit.transform(&against);

        assert!(transformed.is_insert());
        assert_eq!(transformed.text(), "hello");
        assert_eq!(transformed.position(), Position::new(0, 8));
    }

    #[test]
    fn transform_edit_delete_preserves_text() {
        let edit = Edit::delete(Position::new(0, 5), "xyz");
        let against = Edit::insert(Position::new(0, 0), "abc");
        let transformed = edit.transform(&against);

        assert!(transformed.is_delete());
        assert_eq!(transformed.text(), "xyz");
        assert_eq!(transformed.position(), Position::new(0, 8));
    }

    #[test]
    fn transform_edit_before_against_no_shift() {
        let edit = Edit::insert(Position::new(0, 0), "first");
        let against = Edit::insert(Position::new(0, 10), "second");
        let transformed = edit.transform(&against);

        assert_eq!(transformed.position(), Position::new(0, 0));
    }

    #[test]
    fn transform_edit_against_empty_noop() {
        let edit = Edit::insert(Position::new(0, 5), "text");
        let against = Edit::insert(Position::new(0, 0), "");
        let transformed = edit.transform(&against);

        assert_eq!(transformed.position(), Position::new(0, 5));
    }

    #[test]
    fn transform_delete_edit_against_delete() {
        let edit = Edit::delete(Position::new(0, 10), "removed");
        let against = Edit::delete(Position::new(0, 2), "abc");
        let transformed = edit.transform(&against);

        // 10 - 3 = 7
        assert_eq!(transformed.position(), Position::new(0, 7));
    }
}
