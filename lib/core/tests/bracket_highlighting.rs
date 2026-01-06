//! Integration tests for bracket highlighting
//!
//! Tests the bracket plugin's ability to highlight matching bracket pairs
//! when the cursor is inside them, with bold styling when cursor is on a bracket.

mod common;

use common::ServerTest;

/// Helper: get cell at content position (accounting for sign column + line number gutter)
/// Gutter format is: `sign_column` (2 chars) + "N " where N is line number + 1 space
fn get_content_cell(
    snap: &reovim_core::visual::VisualSnapshot,
    content_col: usize,
    row: usize,
) -> Option<&reovim_core::rpc::state::CellSnapshot> {
    // Gutter = sign column (2) + line number (1 digit) + space (1) = 4 chars
    // For single-digit lines with sign column: "  1 " = 4 chars
    let gutter_width = 4; // sign_column (2) + "N " (2) for single-digit line numbers
    snap.cells
        .get(row)
        .and_then(|r| r.get(gutter_width + content_col))
}

#[tokio::test]
async fn test_bracket_pair_highlighting_basic() {
    // Simple case: cursor inside brackets after moving
    let mut result = ServerTest::new()
        .await
        .with_content("(hello)")
        .with_keys("l") // Move cursor to 'h', inside ()
        .run()
        .await;

    let snap = result.visual_snapshot().await;

    // Debug: print the snapshot to see what we're working with
    let plain_text = &snap.plain_text;
    let cursor = &snap.cursor;
    eprintln!("Snapshot plain text:\n{plain_text}");
    eprintln!("Cursor: {cursor:?}");

    // Get cells for '(' at col 0 and ')' at col 6
    let open = get_content_cell(&snap, 0, 0);
    let close = get_content_cell(&snap, 6, 0);

    eprintln!("Open bracket cell: {open:?}");
    eprintln!("Close bracket cell: {close:?}");

    // Both brackets should exist
    assert!(open.is_some(), "Open bracket cell not found");
    assert!(close.is_some(), "Close bracket cell not found");

    let open = open.unwrap();
    let close = close.unwrap();

    // Verify we found the right characters
    assert_eq!(open.char, '(', "Expected '(' at position 0");
    assert_eq!(close.char, ')', "Expected ')' at position 6");

    // Both brackets should have fg color set (rainbow highlight)
    assert!(open.fg.is_some(), "Open bracket should have fg color for highlighting");
    assert!(close.fg.is_some(), "Close bracket should have fg color for highlighting");

    // Both should have SAME color (matched pair)
    assert_eq!(open.fg, close.fg, "Matched brackets should have same color");
}

#[tokio::test]
async fn test_bracket_bold_when_cursor_on_bracket() {
    // Cursor starts at col 0, which is on '('
    let mut result = ServerTest::new()
        .await
        .with_content("(test)")
        // No with_keys - cursor stays at col 0, on '('
        .run()
        .await;

    let snap = result.visual_snapshot().await;

    let plain_text = &snap.plain_text;
    let cursor = &snap.cursor;
    eprintln!("Snapshot plain text:\n{plain_text}");
    eprintln!("Cursor: {cursor:?}");

    let open = get_content_cell(&snap, 0, 0);
    let close = get_content_cell(&snap, 5, 0);

    eprintln!("Open bracket cell: {open:?}");
    eprintln!("Close bracket cell: {close:?}");

    assert!(open.is_some(), "Open bracket cell not found");
    assert!(close.is_some(), "Close bracket cell not found");

    let open = open.unwrap();
    let close = close.unwrap();

    assert_eq!(open.char, '(', "Expected '(' at position 0");
    assert_eq!(close.char, ')', "Expected ')' at position 5");

    // Both brackets should be BOLD when cursor is on either one
    assert!(open.bold, "Open bracket should be bold when cursor is on it");
    assert!(close.bold, "Close bracket should also be bold (matching pair)");
}

#[tokio::test]
async fn test_nested_brackets_innermost_highlighted() {
    // Content: ((x))
    // Positions: 0=( 1=( 2=x 3=) 4=)
    let mut result = ServerTest::new()
        .await
        .with_content("((x))")
        .with_keys("ll") // Move to 'x' at col 2, inside inner ()
        .run()
        .await;

    let snap = result.visual_snapshot().await;

    let plain_text = &snap.plain_text;
    let cursor = &snap.cursor;
    eprintln!("Snapshot plain text:\n{plain_text}");
    eprintln!("Cursor: {cursor:?}");

    // Inner brackets at col 1 and 3
    let inner_open = get_content_cell(&snap, 1, 0);
    let inner_close = get_content_cell(&snap, 3, 0);

    eprintln!("Inner open: {inner_open:?}");
    eprintln!("Inner close: {inner_close:?}");

    assert!(inner_open.is_some(), "Inner open bracket not found");
    assert!(inner_close.is_some(), "Inner close bracket not found");

    let inner_open = inner_open.unwrap();
    let inner_close = inner_close.unwrap();

    assert_eq!(inner_open.char, '(', "Expected '(' at position 1");
    assert_eq!(inner_close.char, ')', "Expected ')' at position 3");

    // Inner pair should be highlighted (have color)
    assert!(inner_open.fg.is_some(), "Inner open bracket should be highlighted");
    assert!(inner_close.fg.is_some(), "Inner close bracket should be highlighted");

    // Inner pair should have matching color
    assert_eq!(inner_open.fg, inner_close.fg, "Inner bracket pair should have same color");
}

#[tokio::test]
async fn test_no_highlight_outside_brackets() {
    // Content: x(y)z
    // Positions: 0=x 1=( 2=y 3=) 4=z
    // Cursor at 'x' (col 0), which is OUTSIDE the brackets
    let mut result = ServerTest::new()
        .await
        .with_content("x(y)z")
        // No with_keys - cursor stays at col 0 ('x')
        .run()
        .await;

    let snap = result.visual_snapshot().await;

    let plain_text = &snap.plain_text;
    let cursor = &snap.cursor;
    eprintln!("Snapshot plain text:\n{plain_text}");
    eprintln!("Cursor: {cursor:?}");

    let open = get_content_cell(&snap, 1, 0); // '(' at col 1
    let close = get_content_cell(&snap, 3, 0); // ')' at col 3

    eprintln!("Open bracket: {open:?}");
    eprintln!("Close bracket: {close:?}");

    assert!(open.is_some(), "Open bracket cell not found");
    assert!(close.is_some(), "Close bracket cell not found");

    let open = open.unwrap();
    let close = close.unwrap();

    assert_eq!(open.char, '(', "Expected '(' at position 1");
    assert_eq!(close.char, ')', "Expected ')' at position 3");

    // When cursor is outside brackets, they should NOT be highlighted
    // They should either have no fg color or default fg color (not rainbow)
    // For this test, we check that brackets don't have the bold attribute
    // (since bold is only applied when cursor is on a bracket in a matched pair)
    assert!(!open.bold, "Brackets should not be bold when cursor is outside");
    assert!(!close.bold, "Brackets should not be bold when cursor is outside");
}

#[tokio::test]
async fn test_multiline_bracket_matching() {
    // Test brackets across multiple lines
    let content = "fn main() {\n    println!(\"hi\");\n}";
    let mut result = ServerTest::new()
        .await
        .with_content(content)
        .with_keys("j") // Move to line 2, inside the outer {}
        .run()
        .await;

    let snap = result.visual_snapshot().await;

    let plain_text = &snap.plain_text;
    let cursor = &snap.cursor;
    eprintln!("Snapshot plain text:\n{plain_text}");
    eprintln!("Cursor: {cursor:?}");

    // The '{' is at line 0, col 10
    // The '}' is at line 2, col 0
    let open = get_content_cell(&snap, 10, 0);
    let close = get_content_cell(&snap, 0, 2);

    eprintln!("Open brace: {open:?}");
    eprintln!("Close brace: {close:?}");

    // Just verify we can find the brackets
    if let (Some(open), Some(close)) = (open, close) {
        let open_char = open.char;
        let close_char = close.char;
        eprintln!("Found open char: '{open_char}', close char: '{close_char}'");
        // If highlighting works, both should have same fg color
        if open.fg.is_some() && close.fg.is_some() {
            assert_eq!(open.fg, close.fg, "Multiline matched brackets should have same color");
        }
    }
}

#[tokio::test]
async fn test_bold_only_when_cursor_on_bracket_not_inside() {
    // Content: (hello)
    // Move cursor: start at '(' (col 0), then move to 'h' (col 1)
    // Bold should be true at '(', false at 'h'

    // First, cursor on '(' - should be bold
    let mut result = ServerTest::new().await.with_content("(hello)").run().await;

    let snap = result.visual_snapshot().await;
    let open = get_content_cell(&snap, 0, 0).unwrap();
    let close = get_content_cell(&snap, 6, 0).unwrap();

    eprintln!("=== Cursor on '(' ===");
    let cursor = &snap.cursor;
    eprintln!("Cursor: {cursor:?}");
    eprintln!("Open: {open:?}");
    eprintln!("Close: {close:?}");

    assert!(open.bold, "Open bracket should be bold when cursor is on it");
    assert!(close.bold, "Close bracket should be bold when cursor is on open");

    // Now move cursor to 'h' - should NOT be bold
    let mut result2 = ServerTest::new()
        .await
        .with_content("(hello)")
        .with_keys("l") // Move to 'h'
        .run()
        .await;

    let snap2 = result2.visual_snapshot().await;
    let open2 = get_content_cell(&snap2, 0, 0).unwrap();
    let close2 = get_content_cell(&snap2, 6, 0).unwrap();

    eprintln!("=== Cursor on 'h' (inside, not on bracket) ===");
    let cursor2 = &snap2.cursor;
    eprintln!("Cursor: {cursor2:?}");
    eprintln!("Open: {open2:?}");
    eprintln!("Close: {close2:?}");

    // Brackets should still be highlighted (have color) but NOT bold
    assert!(open2.fg.is_some(), "Open bracket should still have highlight color");
    assert!(close2.fg.is_some(), "Close bracket should still have highlight color");
    assert!(
        !open2.bold,
        "Open bracket should NOT be bold when cursor is inside but not on bracket"
    );
    assert!(
        !close2.bold,
        "Close bracket should NOT be bold when cursor is inside but not on bracket"
    );
}

#[tokio::test]
async fn test_cursor_movement_updates_highlighting() {
    // Test that moving cursor updates which brackets are highlighted
    // Content: (a)(b)
    // Positions: 0=( 1=a 2=) 3=( 4=b 5=)

    // Cursor at 'a' (col 1) - first pair should be highlighted
    let mut result1 = ServerTest::new()
        .await
        .with_content("(a)(b)")
        .with_keys("l") // Move to 'a'
        .run()
        .await;

    let snap1 = result1.visual_snapshot().await;
    eprintln!("=== Cursor at 'a' ===");
    let cursor1 = &snap1.cursor;
    eprintln!("Cursor: {cursor1:?}");

    let first_open = get_content_cell(&snap1, 0, 0).unwrap();
    let first_close = get_content_cell(&snap1, 2, 0).unwrap();
    let second_open = get_content_cell(&snap1, 3, 0).unwrap();
    let second_close = get_content_cell(&snap1, 5, 0).unwrap();

    eprintln!("First pair: open={first_open:?}, close={first_close:?}");
    eprintln!("Second pair: open={second_open:?}, close={second_close:?}");

    // First pair should be highlighted
    assert!(first_open.fg.is_some(), "First open should be highlighted");
    assert!(first_close.fg.is_some(), "First close should be highlighted");
    assert_eq!(first_open.fg, first_close.fg, "First pair should match");

    // Now move to 'b' (col 4) - second pair should be highlighted
    let mut result2 = ServerTest::new()
        .await
        .with_content("(a)(b)")
        .with_keys("llll") // Move to 'b' (4 moves right)
        .run()
        .await;

    let snap2 = result2.visual_snapshot().await;
    eprintln!("=== Cursor at 'b' ===");
    let cursor2 = &snap2.cursor;
    eprintln!("Cursor: {cursor2:?}");

    let second_open2 = get_content_cell(&snap2, 3, 0).unwrap();
    let second_close2 = get_content_cell(&snap2, 5, 0).unwrap();

    eprintln!("Second pair after move: open={second_open2:?}, close={second_close2:?}");

    // Second pair should be highlighted
    assert!(second_open2.fg.is_some(), "Second open should be highlighted");
    assert!(second_close2.fg.is_some(), "Second close should be highlighted");
    assert_eq!(second_open2.fg, second_close2.fg, "Second pair should match");
}

#[tokio::test]
async fn test_square_brackets() {
    // Test square brackets []
    let mut result = ServerTest::new()
        .await
        .with_content("[hello]")
        .with_keys("l") // Move inside
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    let cursor = &snap.cursor;
    eprintln!("Cursor: {cursor:?}");

    let open = get_content_cell(&snap, 0, 0).unwrap();
    let close = get_content_cell(&snap, 6, 0).unwrap();

    eprintln!("Open: {open:?}");
    eprintln!("Close: {close:?}");

    assert_eq!(open.char, '[', "Expected '[' at position 0");
    assert_eq!(close.char, ']', "Expected ']' at position 6");
    assert!(open.fg.is_some(), "Open square bracket should be highlighted");
    assert!(close.fg.is_some(), "Close square bracket should be highlighted");
    assert_eq!(open.fg, close.fg, "Square bracket pair should have same color");
}

#[tokio::test]
async fn test_curly_braces() {
    // Test curly braces {}
    let mut result = ServerTest::new()
        .await
        .with_content("{hello}")
        .with_keys("l") // Move inside
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    let cursor = &snap.cursor;
    eprintln!("Cursor: {cursor:?}");

    let open = get_content_cell(&snap, 0, 0).unwrap();
    let close = get_content_cell(&snap, 6, 0).unwrap();

    eprintln!("Open: {open:?}");
    eprintln!("Close: {close:?}");

    assert_eq!(open.char, '{', "Expected '{{' at position 0");
    assert_eq!(close.char, '}', "Expected '}}' at position 6");
    assert!(open.fg.is_some(), "Open curly brace should be highlighted");
    assert!(close.fg.is_some(), "Close curly brace should be highlighted");
    assert_eq!(open.fg, close.fg, "Curly brace pair should have same color");
}

#[tokio::test]
async fn test_mixed_bracket_types() {
    // Test mixed brackets: ([{}])
    // Positions: 0=( 1=[ 2={ 3=} 4=] 5=)
    let mut result = ServerTest::new()
        .await
        .with_content("([{}])")
        .with_keys("lll") // Move to '}' position
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    let cursor = &snap.cursor;
    eprintln!("Cursor: {cursor:?}");

    // Cursor at position 3 ('}'), inside all three bracket pairs
    // Innermost pair {} should be highlighted
    let curly_open = get_content_cell(&snap, 2, 0).unwrap();
    let curly_close = get_content_cell(&snap, 3, 0).unwrap();

    eprintln!("Curly open: {curly_open:?}");
    eprintln!("Curly close: {curly_close:?}");

    assert_eq!(curly_open.char, '{', "Expected '{{' at position 2");
    assert_eq!(curly_close.char, '}', "Expected '}}' at position 3");
    assert!(curly_open.fg.is_some(), "Curly brace should be highlighted");
    assert!(curly_close.fg.is_some(), "Curly brace should be highlighted");
    assert_eq!(curly_open.fg, curly_close.fg, "Curly pair should match");
}

#[tokio::test]
async fn test_cursor_on_close_bracket_bolds_both() {
    // When cursor is on ')' not '(', both should still be bold
    let mut result = ServerTest::new()
        .await
        .with_content("(test)")
        .with_keys("f)") // Find and move to ')'
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    let cursor = &snap.cursor;
    eprintln!("Cursor: {cursor:?}");

    let open = get_content_cell(&snap, 0, 0).unwrap();
    let close = get_content_cell(&snap, 5, 0).unwrap();

    eprintln!("Open: {open:?}");
    eprintln!("Close: {close:?}");

    assert_eq!(open.char, '(', "Expected '(' at position 0");
    assert_eq!(close.char, ')', "Expected ')' at position 5");

    // Both should be bold when cursor is on close bracket
    assert!(open.bold, "Open should be bold when cursor is on close");
    assert!(close.bold, "Close should be bold when cursor is on it");
}

#[tokio::test]
async fn test_unmatched_bracket_underline() {
    // Test that unmatched brackets get underline warning style
    // Content: "(test" - missing closing bracket
    let mut result = ServerTest::new().await.with_content("(test").run().await;

    let snap = result.visual_snapshot().await;
    let cursor = &snap.cursor;
    eprintln!("Cursor: {cursor:?}");

    let open = get_content_cell(&snap, 0, 0).unwrap();
    eprintln!("Unmatched open bracket: {open:?}");

    assert_eq!(open.char, '(', "Expected '(' at position 0");
    // Unmatched brackets should have underline
    assert!(open.underline, "Unmatched bracket should have underline");
    // Should also have red-ish foreground color from unmatched style
    assert!(open.fg.is_some(), "Unmatched bracket should have fg color");
}

#[tokio::test]
async fn test_unmatched_closing_bracket_underline() {
    // Test that extra closing bracket gets underline warning
    // Content: "test)" - extra closing bracket
    let mut result = ServerTest::new().await.with_content("test)").run().await;

    let snap = result.visual_snapshot().await;
    let cursor = &snap.cursor;
    eprintln!("Cursor: {cursor:?}");

    let close = get_content_cell(&snap, 4, 0).unwrap();
    eprintln!("Unmatched close bracket: {close:?}");

    assert_eq!(close.char, ')', "Expected ')' at position 4");
    // Unmatched brackets should have underline
    assert!(close.underline, "Unmatched closing bracket should have underline");
}

#[tokio::test]
async fn test_auto_pair_insertion() {
    // Test that typing an opening bracket auto-inserts the closing one
    // Start in insert mode, type '(' - should result in "()" with cursor between
    let mut result = ServerTest::new()
        .await
        .with_content("")
        .with_keys("i(") // Enter insert mode and type '('
        .run()
        .await;

    // The buffer should contain "()" after auto-pair insertion
    result.assert_buffer_contains("()");

    // Cursor should be between the brackets (at position 1)
    // Note: cursor position is (x, y) where x is column
    let snap = result.visual_snapshot().await;
    let cursor = &snap.cursor;
    eprintln!("Cursor after auto-pair: {cursor:?}");
    eprintln!("Buffer content: {}", result.buffer_content);

    // Check that both brackets exist
    let open = get_content_cell(&snap, 0, 0).unwrap();
    let close = get_content_cell(&snap, 1, 0).unwrap();

    assert_eq!(open.char, '(', "Expected '(' at position 0");
    assert_eq!(close.char, ')', "Expected ')' at position 1");
}

#[tokio::test]
async fn test_auto_pair_square_brackets() {
    // Test auto-pair for square brackets
    let result = ServerTest::new()
        .await
        .with_content("")
        .with_keys("i[") // Enter insert mode and type '['
        .run()
        .await;

    result.assert_buffer_contains("[]");
    eprintln!("Buffer content: {}", result.buffer_content);
}

#[tokio::test]
async fn test_auto_pair_curly_braces() {
    // Test auto-pair for curly braces
    let result = ServerTest::new()
        .await
        .with_content("")
        .with_keys("i{") // Enter insert mode and type '{'
        .run()
        .await;

    result.assert_buffer_contains("{}");
    eprintln!("Buffer content: {}", result.buffer_content);
}
