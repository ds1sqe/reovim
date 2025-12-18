//! Word motion integration tests
//!
//! Tests for w, b, e word motions using the server-based test harness.
//!
//! ## Implementation Notes
//!
//! - `e` (word end) motion IS implemented
//! - Count prefix (e.g., `2w`) IS supported
//! - Cross-line movement IS supported
//! - Only whitespace is treated as word boundary (punctuation is part of word)

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

    // At last word on single line, stays at last char (position 4)
    result.assert_cursor(4, 0);
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
async fn test_w_crosses_lines() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("w")
        .run()
        .await;

    // w crosses to next line when at end of current line
    result.assert_cursor(0, 1);
}

#[tokio::test]
async fn test_w_whitespace_only() {
    let result = ServerTest::new()
        .await
        .with_content("   ")
        .with_keys("w")
        .run()
        .await;

    // Whitespace only, stays at last char (col 2)
    result.assert_cursor(2, 0);
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
async fn test_b_crosses_lines() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("jb") // go to line 2, then b
        .run()
        .await;

    // b crosses to previous line when at start of current line
    result.assert_cursor(0, 0);
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
// Count prefix tests
// ============================================================================

/// Count prefix works for w motion
#[tokio::test]
async fn test_count_w() {
    let result = ServerTest::new()
        .await
        .with_content("one two three four")
        .with_keys("2w")
        .run()
        .await;

    // 2w moves forward two words to "three" (col 8)
    result.assert_cursor(8, 0);
}

/// Count prefix works for b motion
#[tokio::test]
async fn test_count_b() {
    let result = ServerTest::new()
        .await
        .with_content("one two three four")
        .with_keys("www2b") // go to "four", then 2b
        .run()
        .await;

    // 2b from "four" goes back two words to "two" (col 4)
    result.assert_cursor(4, 0);
}

// ============================================================================
// Cross-line tests
// ============================================================================

/// w crosses lines to next word
#[tokio::test]
async fn test_w_cross_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("w")
        .run()
        .await;

    // w crosses to "world" on line 2 (col 0, line 1)
    result.assert_cursor(0, 1);
}

/// b crosses lines to previous word
#[tokio::test]
async fn test_b_cross_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("jb") // go to line 2, then b
        .run()
        .await;

    // b crosses to "hello" on line 1 (col 0, line 0)
    result.assert_cursor(0, 0);
}

// ============================================================================
// Documentation tests - Document current behavior
// ============================================================================

/// Documents that punctuation is part of word (not separate)
#[tokio::test]
async fn test_doc_punctuation_not_word_boundary() {
    let result = ServerTest::new()
        .await
        .with_content("hello, world")
        .with_keys("w")
        .run()
        .await;

    // Note: "hello," treated as one word due to whitespace-only boundaries
    // Standard vim would treat comma as separate word
    result.assert_cursor(7, 0);
}
