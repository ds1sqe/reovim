//! Integration tests for % (matching bracket) motion
//!
//! Tests the `Motion::MatchingBracket` implementation for jumping between
//! matching bracket pairs like `()`, `[]`, `{}`.

mod common;

use common::ServerTest;

// ============================================================================
// Basic bracket matching tests
// ============================================================================

#[tokio::test]
async fn test_percent_on_open_paren() {
    // Cursor on '(' should jump to matching ')'
    let result = ServerTest::new()
        .await
        .with_content("(hello)")
        .with_keys("%")
        .run()
        .await;

    // Should jump from '(' at col 0 to ')' at col 6
    result.assert_cursor(6, 0);
}

#[tokio::test]
async fn test_percent_on_close_paren() {
    // Cursor on ')' should jump to matching '('
    let result = ServerTest::new()
        .await
        .with_content("(hello)")
        .with_keys("$%") // Go to end, then %
        .run()
        .await;

    // Should jump from ')' at col 6 to '(' at col 0
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_percent_on_square_brackets() {
    let result = ServerTest::new()
        .await
        .with_content("[test]")
        .with_keys("%")
        .run()
        .await;

    // Should jump from '[' at col 0 to ']' at col 5
    result.assert_cursor(5, 0);
}

#[tokio::test]
async fn test_percent_on_curly_braces() {
    let result = ServerTest::new()
        .await
        .with_content("{test}")
        .with_keys("%")
        .run()
        .await;

    // Should jump from '{' at col 0 to '}' at col 5
    result.assert_cursor(5, 0);
}

// ============================================================================
// Nested bracket tests
// ============================================================================

#[tokio::test]
async fn test_percent_nested_parens() {
    // Content: ((inner))
    // Positions: 0=( 1=( 2-6=inner 7=) 8=)
    let result = ServerTest::new()
        .await
        .with_content("((inner))")
        .with_keys("%")
        .run()
        .await;

    // From outer '(' should jump to outer ')' at col 8
    result.assert_cursor(8, 0);
}

#[tokio::test]
async fn test_percent_inner_nested() {
    // Content: ((inner))
    // Move to inner '(' and then %
    let result = ServerTest::new()
        .await
        .with_content("((inner))")
        .with_keys("l%") // Move to inner '(', then %
        .run()
        .await;

    // From inner '(' at col 1 should jump to inner ')' at col 7
    result.assert_cursor(7, 0);
}

#[tokio::test]
async fn test_percent_mixed_brackets() {
    // Content: ({[]})
    let result = ServerTest::new()
        .await
        .with_content("({[]})")
        .with_keys("%")
        .run()
        .await;

    // From '(' should jump to matching ')'
    result.assert_cursor(5, 0);
}

// ============================================================================
// Search forward behavior (vim-compatible)
// ============================================================================

#[tokio::test]
async fn test_percent_search_forward_for_bracket() {
    // When cursor is not on a bracket, vim searches forward on the current line
    // Content: "hello (world)"
    // Positions: 0-4=hello 5=space 6=( 7-11=world 12=)
    let result = ServerTest::new()
        .await
        .with_content("hello (world)")
        .with_keys("%") // Cursor at 'h', should find '(' and jump to ')'
        .run()
        .await;

    // Should find '(' at col 6 and jump to matching ')' at col 12
    result.assert_cursor(12, 0);
}

#[tokio::test]
async fn test_percent_no_bracket_on_line() {
    // When there's no bracket on the current line, cursor should stay
    let result = ServerTest::new()
        .await
        .with_content("no brackets here")
        .with_keys("%")
        .run()
        .await;

    // Should stay at col 0
    result.assert_cursor(0, 0);
}

// ============================================================================
// Multiline bracket matching
// ============================================================================

#[tokio::test]
async fn test_percent_multiline_forward() {
    let result = ServerTest::new()
        .await
        .with_content("{\n  inner\n}")
        .with_keys("%")
        .run()
        .await;

    // From '{' at (0,0) should jump to '}' at (0,2)
    result.assert_cursor(0, 2);
}

#[tokio::test]
async fn test_percent_multiline_backward() {
    let result = ServerTest::new()
        .await
        .with_content("{\n  inner\n}")
        .with_keys("G%") // Go to last line, then %
        .run()
        .await;

    // From '}' at (0,2) should jump to '{' at (0,0)
    result.assert_cursor(0, 0);
}

// ============================================================================
// Roundtrip tests
// ============================================================================

#[tokio::test]
async fn test_percent_roundtrip() {
    // % twice should return to original position
    let result = ServerTest::new()
        .await
        .with_content("(test)")
        .with_keys("%%")
        .run()
        .await;

    // Should be back at col 0
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_percent_roundtrip_from_close() {
    let result = ServerTest::new()
        .await
        .with_content("(test)")
        .with_keys("$%%")
        .run()
        .await;

    // Should be back at col 5 (the ')' position)
    result.assert_cursor(5, 0);
}

// ============================================================================
// Unmatched bracket tests
// ============================================================================

#[tokio::test]
async fn test_percent_unmatched_open() {
    // Unmatched '(' - should stay at current position
    let result = ServerTest::new()
        .await
        .with_content("(no close")
        .with_keys("%")
        .run()
        .await;

    // Should stay at col 0 (no match found)
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_percent_unmatched_close() {
    // Unmatched ')' - should stay at current position
    let result = ServerTest::new()
        .await
        .with_content("no open)")
        .with_keys("$%") // Go to ')', then %
        .run()
        .await;

    // Should stay at col 7 (no match found)
    result.assert_cursor(7, 0);
}

// ============================================================================
// Complex code patterns
// ============================================================================

#[tokio::test]
async fn test_percent_function_call() {
    let result = ServerTest::new()
        .await
        .with_content("foo(bar, baz)")
        .with_keys("lll%") // Move to '(' at col 3, then % to match
        .run()
        .await;

    // Should jump to ')' at col 12
    result.assert_cursor(12, 0);
}

#[tokio::test]
async fn test_percent_array_index() {
    let result = ServerTest::new()
        .await
        .with_content("arr[i + 1]")
        .with_keys("lll%") // Move to '[' at col 3, then % to match
        .run()
        .await;

    // Should jump to ']' at col 9
    result.assert_cursor(9, 0);
}

#[tokio::test]
async fn test_percent_code_block() {
    let result = ServerTest::new()
        .await
        .with_content("if x {\n    y\n}")
        .with_keys("lllll%") // Move to '{' at col 5, then % to match
        .run()
        .await;

    // Should jump to '}' at (0, 2)
    result.assert_cursor(0, 2);
}
