use {
    super::*,
    crate::{Edit, Position},
};

// === Edit construction (from mm/tests/edit.rs) ===

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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
    let result = transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 2), "abc"));
    assert_eq!(result, Position::new(0, 8)); // 5 + 3
}

// -- Insert: at same position (tie-breaking: left-bias) --

#[test]
fn transform_pos_at_insert_position() {
    let result = transform_position(Position::new(0, 5), &Edit::insert(Position::new(0, 5), "xyz"));
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
    let result = transform_position(Position::new(0, 1), &Edit::delete(Position::new(0, 5), "abc"));
    assert_eq!(result, Position::new(0, 1));
}

// -- Delete: position at delete start is unaffected (tie-breaking) --

#[test]
fn transform_pos_at_delete_start() {
    let result = transform_position(Position::new(0, 5), &Edit::delete(Position::new(0, 5), "abc"));
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
    let result =
        transform_position(Position::new(1, 2), &Edit::delete(Position::new(0, 3), "hello\nworld"));
    assert_eq!(result, Position::new(0, 3));
}

// -- Delete: position after single-line delete shifts left --

#[test]
fn transform_pos_after_single_line_delete() {
    let result = transform_position(Position::new(0, 8), &Edit::delete(Position::new(0, 2), "abc"));
    assert_eq!(result, Position::new(0, 5)); // 8 - 3
}

// -- Delete: position on last line of multiline delete, after delete end --

#[test]
fn transform_pos_after_multiline_delete_same_end_line() {
    // Delete "hello\nworld" at (0,3), del_end = (1,5)
    // Position (1,8) -> (0, 3 + 8 - 5) = (0, 6)
    let result =
        transform_position(Position::new(1, 8), &Edit::delete(Position::new(0, 3), "hello\nworld"));
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

// === Edit Tests (from mm/tests/mod.rs inline edit_tests) ===

mod edit_tests_inline {
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

// === OT-Lite Position Transformation Tests (from mm/tests/mod.rs inline transform_tests) ===

mod transform_tests_inline {
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
