//! Leap navigation integration tests
//!
//! Tests for s (leap forward), S (leap backward) two-character jump navigation.

mod common;

use common::*;

// ============================================================================
// Entering leap mode
// ============================================================================

#[tokio::test]
async fn test_s_enters_leap_mode() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("s")
        .run()
        .await;

    // s should enter leap mode (sub_mode becomes Leap)
    // Note: Mode display might show "NORMAL" but sub_mode is "Leap"
    assert!(
        result.mode.sub_mode.contains("Leap") || result.mode.display.contains("LEAP"),
        "Expected leap mode, got {:?}",
        result.mode
    );
}

#[tokio::test]
async fn test_big_s_enters_leap_backward() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("$S")
        .run()
        .await;

    // $ to end, S should enter backward leap mode
    assert!(
        result.mode.sub_mode.contains("Leap") || result.mode.display.contains("LEAP"),
        "Expected leap mode, got {:?}",
        result.mode
    );
}

// ============================================================================
// Leap cancellation
// ============================================================================

#[tokio::test]
async fn test_leap_escape_cancels() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("s<Esc>")
        .run()
        .await;

    // Escape should cancel leap and return to normal
    result.assert_normal_mode();
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_leap_backward_escape_cancels() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("$S<Esc>")
        .run()
        .await;

    // Should return to normal mode at same position
    result.assert_normal_mode();
}

// ============================================================================
// Leap with two characters (jump)
// ============================================================================

/// Leap forward with single match auto-jumps to target
#[tokio::test]
async fn test_leap_forward_single_match() {
    let result = ServerTest::new()
        .await
        .with_content("hello world test")
        .with_keys("swo")
        .with_delay(200) // Extra delay for leap processing under parallel test load
        .run()
        .await;

    // Single match "wo" in "world" at col 6 - auto-jumps
    result.assert_cursor(6, 0);
    result.assert_normal_mode();
}

/// Leap with first match after cursor when multiple exist
#[tokio::test]
async fn test_leap_first_match_after_cursor() {
    let result = ServerTest::new()
        .await
        .with_content("test test more")
        .with_keys("ste")
        .with_delay(200) // Extra delay for leap processing under parallel test load
        .run()
        .await;

    // First "te" at col 0 is at cursor, second "te" at col 5 is the match
    // Single match forward from cursor position - auto-jumps to col 5
    result.assert_cursor(5, 0);
    result.assert_normal_mode();
}

// ============================================================================
// Leap no match
// ============================================================================

#[tokio::test]
async fn test_leap_no_match() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("sxy")
        .run()
        .await;

    // "xy" doesn't exist, should stay at original position
    result.assert_cursor(0, 0);
    result.assert_normal_mode();
}

// ============================================================================
// Leap across lines
// ============================================================================

/// Leap forward across lines to single match
#[tokio::test]
async fn test_leap_across_lines() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld\ntest")
        .with_keys("ste")
        .with_delay(200) // Extra delay for leap processing under parallel test load
        .run()
        .await;

    // "te" in "test" is on line 2 (0-indexed), col 0 - auto-jumps
    result.assert_cursor(0, 2);
    result.assert_normal_mode();
}

// ============================================================================
// Leap with operators (experimental)
// ============================================================================

#[tokio::test]
async fn test_leap_preserves_buffer() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("swo")
        .with_delay(200) // Extra delay for leap processing under parallel test load
        .run()
        .await;

    // Leap alone shouldn't modify buffer
    result.assert_buffer_eq("hello world");
}
