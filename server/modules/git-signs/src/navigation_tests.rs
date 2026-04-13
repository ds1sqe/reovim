use reovim_subsys_git::types::DiffHunk;

use super::*;

fn make_hunk(new_start: usize, new_count: usize) -> DiffHunk {
    DiffHunk {
        old_start: 1,
        old_count: 1,
        new_start,
        new_count,
    }
}

// ============================================================================
// next_hunk_line
// ============================================================================

#[test]
fn next_hunk_empty() {
    assert_eq!(next_hunk_line(&[], 5), None);
}

#[test]
fn next_hunk_forward() {
    let hunks = vec![make_hunk(5, 2), make_hunk(15, 3), make_hunk(25, 1)];
    // Cursor at line 0 (0-indexed) → next is hunk at line 4 (5-1)
    assert_eq!(next_hunk_line(&hunks, 0), Some(4));
}

#[test]
fn next_hunk_between() {
    let hunks = vec![make_hunk(5, 2), make_hunk(15, 3), make_hunk(25, 1)];
    // Cursor at line 7 (0-indexed, past first hunk) → next is hunk at line 14 (15-1)
    assert_eq!(next_hunk_line(&hunks, 7), Some(14));
}

#[test]
fn next_hunk_wraps() {
    let hunks = vec![make_hunk(5, 2), make_hunk(15, 3)];
    // Cursor at line 20 → wraps to first hunk at line 4
    assert_eq!(next_hunk_line(&hunks, 20), Some(4));
}

#[test]
fn next_hunk_on_hunk_line() {
    let hunks = vec![make_hunk(5, 2), make_hunk(15, 3)];
    // Cursor at line 4 (0-indexed = hunk.new_start 5, 1-indexed) → stays on same? No, cursor_1indexed = 5 = hunk.new_start, so not > cursor
    // next_hunk looks for new_start > cursor_1indexed(5), first is 5 which is NOT >, second is 15 which IS
    assert_eq!(next_hunk_line(&hunks, 4), Some(14));
}

// ============================================================================
// prev_hunk_line
// ============================================================================

#[test]
fn prev_hunk_empty() {
    assert_eq!(prev_hunk_line(&[], 5), None);
}

#[test]
fn prev_hunk_backward() {
    let hunks = vec![make_hunk(5, 2), make_hunk(15, 3), make_hunk(25, 1)];
    // Cursor at line 20 (0-indexed) → prev is hunk at line 14 (15-1)
    assert_eq!(prev_hunk_line(&hunks, 20), Some(14));
}

#[test]
fn prev_hunk_wraps() {
    let hunks = vec![make_hunk(5, 2), make_hunk(15, 3)];
    // Cursor at line 2 → wraps to last hunk at line 14
    assert_eq!(prev_hunk_line(&hunks, 2), Some(14));
}

#[test]
fn prev_hunk_between() {
    let hunks = vec![make_hunk(5, 2), make_hunk(15, 3), make_hunk(25, 1)];
    // Cursor at line 20 → prev is 14 (15-1)
    assert_eq!(prev_hunk_line(&hunks, 20), Some(14));
}

// ============================================================================
// hunk_at_cursor
// ============================================================================

#[test]
fn hunk_at_cursor_none() {
    let hunks = vec![make_hunk(5, 2)];
    // Cursor at line 0 → not in any hunk
    assert!(hunk_at_cursor(&hunks, 0).is_none());
}

#[test]
fn hunk_at_cursor_inside() {
    let hunks = vec![make_hunk(5, 3)];
    // Cursor at line 4 (0-indexed) = 5 (1-indexed) = hunk start → inside
    assert!(hunk_at_cursor(&hunks, 4).is_some());
    // Cursor at line 6 (0-indexed) = 7 (1-indexed) = hunk start+2 → inside
    assert!(hunk_at_cursor(&hunks, 6).is_some());
}

#[test]
fn hunk_at_cursor_after() {
    let hunks = vec![make_hunk(5, 3)];
    // Cursor at line 7 (0-indexed) = 8 (1-indexed) = past hunk end (5+3=8) → NOT inside (< 8)
    assert!(hunk_at_cursor(&hunks, 7).is_none());
}

#[test]
fn hunk_at_cursor_deletion() {
    let hunks = vec![DiffHunk {
        old_start: 5,
        old_count: 3,
        new_start: 5,
        new_count: 0, // deletion
    }];
    // Deletion hunk marker at line 4 (0-indexed) = 5 (1-indexed)
    assert!(hunk_at_cursor(&hunks, 4).is_some());
}

#[test]
fn hunk_at_cursor_empty_hunks() {
    assert!(hunk_at_cursor(&[], 5).is_none());
}

// ============================================================================
// file_path_from_str
// ============================================================================

#[test]
fn file_path_conversion() {
    let path = file_path_from_str("/src/main.rs");
    assert_eq!(path.to_str(), Some("/src/main.rs"));
}
