//! Tests for the mm module.

use super::*;

// Text types extracted to reovim-types-text (#740)
use reovim_types_text::{
    CharKind, Edit, Selection, SelectionMode, TextDimensions, WordType, char_kind, delete_end,
    next_word_end, next_word_start, text_dimensions, transform_position, word_bounds, word_end,
    word_start,
};

mod buffer_id;
mod cache;
mod edit;
mod position;
mod saturator;
mod selection;
mod word;

// LineIndex tests moved to reovim-types-text (#740)
// PieceTree tests moved to reovim-driver-vfs (#740)
// BufferSnapshot tests moved to reovim-provider-text (#740)
// VirtualBuffer tests moved to reovim-provider-text (#740)

// === BufferId Tests ===

mod buffer_id_tests {
    use super::*;

    #[test]
    fn test_unique_ids() {
        let id1 = BufferId::new();
        let id2 = BufferId::new();
        let id3 = BufferId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_ordering() {
        let id1 = BufferId::new();
        let id2 = BufferId::new();

        assert!(id1 < id2);
    }

    #[test]
    fn test_from_raw() {
        let id = BufferId::from_raw(42);
        assert_eq!(id.as_usize(), 42);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_display() {
        let id = BufferId::from_raw(123);
        assert_eq!(format!("{id}"), "Buffer(123)");
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let id1 = BufferId::new();
        let id2 = BufferId::new();

        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2);
        set.insert(id1); // Duplicate

        assert_eq!(set.len(), 2);
    }
}

// === Position Tests ===

mod position_tests {
    use super::*;

    #[test]
    fn test_origin() {
        let pos = Position::origin();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_new() {
        let pos = Position::new(5, 10);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.column, 10);
    }

    #[test]
    fn test_line_start() {
        let pos = Position::line_start(3);
        assert_eq!(pos.line, 3);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_ordering() {
        assert!(Position::new(0, 0) < Position::new(0, 1));
        assert!(Position::new(0, 1) < Position::new(1, 0));
        assert!(Position::new(1, 5) < Position::new(2, 0));
        assert_eq!(Position::new(1, 1), Position::new(1, 1));
    }

    #[test]
    fn test_ordering_same_line() {
        assert!(Position::new(5, 0) < Position::new(5, 1));
        assert!(Position::new(5, 1) < Position::new(5, 10));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_display() {
        let pos = Position::new(0, 5);
        // Display uses 1-indexed for human readability
        assert_eq!(format!("{pos}"), "1:6");
    }

    #[test]
    fn test_default() {
        let pos = Position::default();
        assert_eq!(pos, Position::origin());
    }
}

// === Cursor Tests ===

mod cursor_tests {
    use super::*;

    #[test]
    fn test_origin() {
        let cursor = Cursor::origin();
        assert_eq!(cursor.position, Position::origin());
        assert!(cursor.anchor.is_none());
        assert!(cursor.preferred_column.is_none());
    }

    #[test]
    fn test_new() {
        let cursor = Cursor::new(Position::new(5, 10));
        assert_eq!(cursor.position, Position::new(5, 10));
        assert!(!cursor.has_selection());
    }

    #[test]
    fn test_selection() {
        let mut cursor = Cursor::new(Position::new(0, 5));
        assert!(!cursor.has_selection());

        cursor.start_selection();
        assert!(cursor.has_selection());
        assert_eq!(cursor.anchor, Some(Position::new(0, 5)));

        cursor.clear_selection();
        assert!(!cursor.has_selection());
    }

    #[test]
    fn test_selection_bounds() {
        let mut cursor = Cursor::new(Position::new(0, 0));
        cursor.start_selection();
        cursor.position = Position::new(0, 5);

        let bounds = cursor.selection_bounds();
        assert_eq!(bounds, Some((Position::new(0, 0), Position::new(0, 5))));
    }

    #[test]
    fn test_selection_bounds_backward() {
        let mut cursor = Cursor::new(Position::new(0, 5));
        cursor.start_selection();
        cursor.position = Position::new(0, 0);

        let bounds = cursor.selection_bounds();
        // Should normalize to (start, end)
        assert_eq!(bounds, Some((Position::new(0, 0), Position::new(0, 5))));
    }

    #[test]
    fn test_selection_bounds_multiline() {
        let mut cursor = Cursor::new(Position::new(0, 5));
        cursor.start_selection();
        cursor.position = Position::new(2, 3);

        let bounds = cursor.selection_bounds();
        assert_eq!(bounds, Some((Position::new(0, 5), Position::new(2, 3))));
    }

    #[test]
    fn test_preferred_column() {
        let mut cursor = Cursor::new(Position::new(0, 10));
        assert!(cursor.preferred_column.is_none());
        assert_eq!(cursor.effective_column(), 10);

        cursor.update_preferred_column();
        assert_eq!(cursor.preferred_column, Some(10));
        assert_eq!(cursor.effective_column(), 10);

        // Change position but preferred_column stays
        cursor.position = Position::new(1, 5);
        assert_eq!(cursor.effective_column(), 10); // Still uses preferred

        cursor.clear_preferred_column();
        assert!(cursor.preferred_column.is_none());
        assert_eq!(cursor.effective_column(), 5); // Now uses actual
    }
}

// === Edit Tests ===

mod edit_tests {
    use super::*;

    #[test]
    fn test_insert_edit() {
        let edit = Edit::insert(Position::new(0, 0), "Hello");
        assert!(edit.is_insert());
        assert!(!edit.is_delete());
        assert_eq!(edit.position(), Position::new(0, 0));
        assert_eq!(edit.text(), "Hello");
    }

    #[test]
    fn test_delete_edit() {
        let edit = Edit::delete(Position::new(1, 5), "World");
        assert!(!edit.is_insert());
        assert!(edit.is_delete());
        assert_eq!(edit.position(), Position::new(1, 5));
        assert_eq!(edit.text(), "World");
    }

    #[test]
    fn test_inverse() {
        let insert = Edit::insert(Position::new(0, 0), "Test");
        let inverse = insert.inverse();

        assert!(inverse.is_delete());
        assert_eq!(inverse.position(), Position::new(0, 0));
        assert_eq!(inverse.text(), "Test");

        // Double inverse returns to original
        let double = inverse.inverse();
        assert_eq!(double, insert);
    }

    #[test]
    fn test_is_empty() {
        let empty_insert = Edit::insert(Position::origin(), "");
        assert!(empty_insert.is_empty());

        let non_empty = Edit::insert(Position::origin(), "x");
        assert!(!non_empty.is_empty());
    }
}

// Buffer tests moved to reovim-provider-text (#740)

// === OT-Lite Position Transformation Tests (#495) ===

mod transform_tests {
    use super::*;

    // --- text_dimensions ---

    #[test]
    fn test_text_dimensions_single_line() {
        let dims = text_dimensions("hello");
        assert_eq!(dims.line_count, 0);
        assert_eq!(dims.last_line_len, 5);
    }

    #[test]
    fn test_text_dimensions_multi_line() {
        let dims = text_dimensions("ab\ncd\ne");
        assert_eq!(dims.line_count, 2);
        assert_eq!(dims.last_line_len, 1);
    }

    #[test]
    fn test_text_dimensions_trailing_newline() {
        let dims = text_dimensions("hello\n");
        assert_eq!(dims.line_count, 1);
        assert_eq!(dims.last_line_len, 0);
    }

    #[test]
    fn test_text_dimensions_empty() {
        let dims = text_dimensions("");
        assert_eq!(dims.line_count, 0);
        assert_eq!(dims.last_line_len, 0);
    }

    // --- transform_position against Insert ---

    #[test]
    fn test_transform_position_insert_before() {
        // Position is before the insert: unchanged.
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 10), "abc")),
            Position::new(0, 5)
        );
        // Different line, before.
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(1, 0), "abc")),
            Position::new(0, 5)
        );
    }

    #[test]
    fn test_transform_position_insert_after_same_line() {
        // Insert "abc" at (0,2), position (0,5) shifts to (0,8).
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 2), "abc")),
            Position::new(0, 8)
        );
    }

    #[test]
    fn test_transform_position_insert_after_different_line() {
        // Insert two lines at (1,0), position (3,5) shifts to (5,5).
        assert_eq!(
            transform_position(Position::new(3, 5), &Edit::insert(Position::new(1, 0), "aa\nbb\n")),
            Position::new(5, 5) // 3 + 2 newlines = 5
        );
    }

    #[test]
    fn test_transform_position_insert_multiline() {
        // Insert "ab\nc" at (0,2), position (0,5) moves to (1, 5-2+1 = 4).
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 2), "ab\nc")),
            Position::new(1, 4)
        );
    }

    #[test]
    fn test_transform_position_same_position_insert() {
        // Tie-breaking: insert at same position shifts right (left-bias).
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 5), "abc")),
            Position::new(0, 8) // 5 + 3
        );
    }

    // --- transform_position against Delete ---

    #[test]
    fn test_transform_position_delete_before() {
        // Position is before the delete: unchanged.
        assert_eq!(
            transform_position(Position::new(0, 1), &Edit::delete(Position::new(0, 5), "abc")),
            Position::new(0, 1)
        );
    }

    #[test]
    fn test_transform_position_delete_within() {
        // Position is within deleted range: collapses to delete start.
        assert_eq!(
            transform_position(Position::new(0, 3), &Edit::delete(Position::new(0, 2), "abcde")),
            Position::new(0, 2)
        );
    }

    #[test]
    fn test_transform_position_delete_after_same_line() {
        // Delete "abc" at (0,2), position (0,5) shifts to (0,2).
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::delete(Position::new(0, 2), "abc")),
            Position::new(0, 2)
        );
        // Position further after the deleted range.
        assert_eq!(
            transform_position(Position::new(0, 8), &Edit::delete(Position::new(0, 2), "abc")),
            Position::new(0, 5) // 8 - 3 = 5
        );
    }

    #[test]
    fn test_transform_position_delete_after_different_line() {
        // Delete two lines starting at (1,0), position (5,3) shifts to (3,3).
        assert_eq!(
            transform_position(
                Position::new(5, 3),
                &Edit::delete(Position::new(1, 0), "hello\nworld\n")
            ),
            Position::new(3, 3) // 5 - 2 newlines = 3
        );
    }

    #[test]
    fn test_transform_position_delete_multiline() {
        // Multiline delete: position on last line of deletion but after it.
        // Delete "hello\nworld" at (0,3), del_end=(1,5).
        // Position (1,8) is on the last line, after del_end.
        assert_eq!(
            transform_position(
                Position::new(1, 8),
                &Edit::delete(Position::new(0, 3), "hello\nworld")
            ),
            Position::new(0, 3 + 8 - 5) // del_pos.col + pos.col - del_end.col = 3+8-5 = 6
        );
    }

    #[test]
    fn test_transform_position_delete_multiline_within() {
        // Position within multiline delete: collapses to delete start.
        assert_eq!(
            transform_position(
                Position::new(1, 5),
                &Edit::delete(Position::new(0, 3), "hello\nworld")
            ),
            Position::new(0, 3)
        );
    }

    #[test]
    fn test_transform_position_same_position_delete() {
        // Tie-breaking: delete at same position leaves pos unchanged.
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::delete(Position::new(0, 5), "abc")),
            Position::new(0, 5) // pos <= del_pos, unchanged
        );
    }

    // --- Empty edit ---

    #[test]
    fn test_transform_position_empty_edit() {
        // Empty insert and delete are no-ops.
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 2), "")),
            Position::new(0, 5)
        );
        assert_eq!(
            transform_position(Position::new(0, 5), &Edit::delete(Position::new(0, 2), "")),
            Position::new(0, 5)
        );
    }

    // --- Edit::transform ---

    #[test]
    fn test_transform_edit_preserves_text() {
        let edit = Edit::insert(Position::new(0, 5), "hello");
        let against = Edit::insert(Position::new(0, 2), "abc");
        let transformed = edit.transform(&against);

        assert_eq!(transformed.position(), Position::new(0, 8));
        assert_eq!(transformed.text(), "hello"); // text unchanged
        assert!(transformed.is_insert());
    }

    #[test]
    fn test_transform_edit_delete() {
        let edit = Edit::delete(Position::new(0, 5), "xyz");
        let against = Edit::insert(Position::new(0, 0), "abc");
        let transformed = edit.transform(&against);

        assert_eq!(transformed.position(), Position::new(0, 8)); // 5 + 3
        assert_eq!(transformed.text(), "xyz"); // text unchanged
        assert!(transformed.is_delete());
    }

    #[test]
    fn test_transform_identity_no_shift() {
        // Position before the intervening edit: no shift.
        let edit = Edit::insert(Position::new(0, 0), "hello");
        let against = Edit::insert(Position::new(0, 10), "abc");
        let transformed = edit.transform(&against);

        assert_eq!(transformed.position(), Position::new(0, 0));
        assert_eq!(transformed.text(), "hello");
    }

    // --- delete_end ---

    #[test]
    fn test_delete_end_single_line() {
        assert_eq!(delete_end(Position::new(0, 3), "abc"), Position::new(0, 6));
    }

    #[test]
    fn test_delete_end_multi_line() {
        assert_eq!(delete_end(Position::new(0, 3), "hello\nworld"), Position::new(1, 5));
    }

    #[test]
    fn test_delete_end_trailing_newline() {
        assert_eq!(delete_end(Position::new(2, 0), "hello\n"), Position::new(3, 0));
    }
}
