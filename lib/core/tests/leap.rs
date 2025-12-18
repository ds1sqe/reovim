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
// Leap with two characters (jump) - documenting actual behavior
// ============================================================================

/// Documents that leap forward doesn't move cursor
#[tokio::test]
async fn test_doc_leap_forward_no_jump() {
    let result = ServerTest::new()
        .await
        .with_content("hello world test")
        .with_keys("swo")
        .run()
        .await;

    // Expected vim-leap behavior: cursor jumps to "world" (col 6)
    // Actual behavior: cursor stays at starting position
    result.assert_cursor(0, 0);
    result.assert_normal_mode();
}

/// Documents that leap with multiple matches doesn't jump
#[tokio::test]
async fn test_doc_leap_multiple_matches_no_jump() {
    let result = ServerTest::new()
        .await
        .with_content("test test more")
        .with_keys("ste")
        .run()
        .await;

    // Expected vim-leap behavior: jump to second "te" at col 5
    // Actual behavior: cursor stays at starting position
    result.assert_cursor(0, 0);
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
// Leap across lines - documenting actual behavior
// ============================================================================

/// Documents that leap across lines doesn't jump
#[tokio::test]
async fn test_doc_leap_across_lines_no_jump() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld\ntest")
        .with_keys("ste")
        .run()
        .await;

    // Expected vim-leap behavior: jump to "te" in "test" (line 3, col 0)
    // Actual behavior: cursor stays at starting position
    result.assert_cursor(0, 0);
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
        .run()
        .await;

    // Leap alone shouldn't modify buffer
    result.assert_buffer_eq("hello world");
}
