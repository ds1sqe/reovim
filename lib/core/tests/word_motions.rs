//! Word motion integration tests
//!
//! Tests for w, b word motions using the server-based test harness.
//!
//! ## Current Implementation Notes
//!
//! The word motion implementation has the following limitations:
//! - `e` (word end) motion is NOT implemented
//! - Count prefix (e.g., `2w`) is NOT supported - each motion moves one word
//! - Cross-line movement is NOT supported - motions stay on current line
//! - Only whitespace is treated as word boundary (punctuation is part of word)
//!
//! Tests are written to verify current behavior. Tests marked with `_actual_vim_`
//! prefix document where behavior differs from standard vim.

mod common;

use common::*;

// ============================================================================
// w (word forward) tests - Current Implementation
// ============================================================================

#[tokio::test]
async fn test_w_single_word() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("w")
        .run()
        .await;

    // Moves to start of "world" (column 6)
    result.assert_cursor(6, 0);
}

#[tokio::test]
async fn test_w_multiple_words() {
    let result = ServerTest::new()
        .await
        .with_content("one two three")
        .with_keys("ww")
        .run()
        .await;

    // Two w motions: 'o' -> 't'(two) -> 't'(three)
    result.assert_cursor(8, 0);
}

#[tokio::test]
async fn test_w_empty_buffer() {
    let result = ServerTest::new()
        .await
        .with_content("")
        .with_keys("w")
        .run()
        .await;

    // Empty buffer, cursor stays at 0,0
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_w_single_char_words() {
    let result = ServerTest::new()
        .await
        .with_content("a b c d")
        .with_keys("www")
        .run()
        .await;

    // Each letter is a word: 'a' -> 'b' -> 'c' -> 'd'
    result.assert_cursor(6, 0);
}

#[tokio::test]
async fn test_w_at_last_word() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("w")
        .run()
        .await;

    // At last word, goes to past end (position 5)
    // Current implementation doesn't clamp to last char
    result.assert_cursor(5, 0);
}

#[tokio::test]
async fn test_w_with_punctuation() {
    let result = ServerTest::new()
        .await
        .with_content("hello, world")
        .with_keys("w")
        .run()
        .await;

    // Current impl: whitespace-only boundary, so "hello," is one word
    // w goes to "world" (column 7)
    result.assert_cursor(7, 0);
}

#[tokio::test]
async fn test_w_stays_on_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("w")
        .run()
        .await;

    // Current impl: w doesn't cross lines, stays at end of "hello" (col 5)
    result.assert_cursor(5, 0);
}

#[tokio::test]
async fn test_w_whitespace_only() {
    let result = ServerTest::new()
        .await
        .with_content("   ")
        .with_keys("w")
        .run()
        .await;

    // Whitespace only, goes past end (col 3)
    result.assert_cursor(3, 0);
}

// ============================================================================
// b (word backward) tests - Current Implementation
// ============================================================================

#[tokio::test]
async fn test_b_single_word() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("wb") // w to "world", then b back
        .run()
        .await;

    // b from "world" goes back to "hello"
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_b_from_middle_of_word() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("wllb") // w to "world", ll to "rld", b to start of "world"
        .run()
        .await;

    // b from middle of "world" goes to start of "world"
    result.assert_cursor(6, 0);
}

#[tokio::test]
async fn test_b_multiple_words() {
    let result = ServerTest::new()
        .await
        .with_content("one two three")
        .with_keys("wwbb") // go to "three", then back twice
        .run()
        .await;

    // Two b motions from "three": -> "two" -> "one"
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_b_at_start() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("b")
        .run()
        .await;

    // Already at start, b stays at 0,0
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_b_stays_on_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("jb") // go to line 2, then b
        .run()
        .await;

    // Current impl: b doesn't cross lines, stays at start of "world" (col 0, line 1)
    result.assert_cursor(0, 1);
}

// ============================================================================
// Roundtrip tests
// ============================================================================

#[tokio::test]
async fn test_w_b_roundtrip() {
    let result = ServerTest::new()
        .await
        .with_content("hello world test")
        .with_keys("wwb") // w to "world", w to "test", b back to "world"
        .run()
        .await;

    // Should be at "world" (col 6)
    result.assert_cursor(6, 0);
}

#[tokio::test]
async fn test_multiple_w_b_cycles() {
    let result = ServerTest::new()
        .await
        .with_content("one two three four")
        .with_keys("wwwbbb") // forward 3, back 3
        .run()
        .await;

    // Should be back at start
    result.assert_cursor(0, 0);
}

// ============================================================================
// Documentation tests - These document current limitations
// ============================================================================

/// Documents that count prefix doesn't work for w motion
#[tokio::test]
async fn test_doc_count_not_supported_w() {
    let result = ServerTest::new()
        .await
        .with_content("one two three four")
        .with_keys("2w")
        .run()
        .await;

    // Expected vim behavior: 2w -> "three" (col 8)
    // Actual behavior: count ignored, acts like single w -> "two" (col 4)
    result.assert_cursor(4, 0);
}

/// Documents that count prefix doesn't work for b motion
#[tokio::test]
async fn test_doc_count_not_supported_b() {
    let result = ServerTest::new()
        .await
        .with_content("one two three four")
        .with_keys("www2b") // go to "four", then try 2b
        .run()
        .await;

    // Expected vim behavior: 2b from "four" -> "two" (col 4)
    // Actual behavior: count ignored, single b -> "three" (col 8)
    result.assert_cursor(8, 0);
}

/// Documents that w doesn't cross lines
#[tokio::test]
async fn test_doc_w_no_cross_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("w")
        .run()
        .await;

    // Expected vim behavior: w -> "world" on line 2 (col 0, line 1)
    // Actual behavior: stays on line 1, goes to end of "hello" (col 5, line 0)
    result.assert_cursor(5, 0);
}

/// Documents that b doesn't cross lines
#[tokio::test]
async fn test_doc_b_no_cross_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("jb") // go to line 2, then b
        .run()
        .await;

    // Expected vim behavior: b -> "hello" on line 1 (col 0, line 0)
    // Actual behavior: stays on line 2, at start of "world" (col 0, line 1)
    result.assert_cursor(0, 1);
}

/// Documents that punctuation is part of word (not separate)
#[tokio::test]
async fn test_doc_punctuation_not_word_boundary() {
    let result = ServerTest::new()
        .await
        .with_content("hello, world")
        .with_keys("w")
        .run()
        .await;

    // Expected vim behavior: w -> ',' (col 5) - comma is separate word
    // Actual behavior: "hello," treated as one word, goes to "world" (col 7)
    result.assert_cursor(7, 0);
}
