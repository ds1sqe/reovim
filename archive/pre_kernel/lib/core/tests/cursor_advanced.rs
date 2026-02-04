//! Advanced cursor motion integration tests
//!
//! Tests for gg, G, 0, $, and count prefix motions.

mod common;

use common::*;

// ============================================================================
// gg (go to first line)
// ============================================================================

#[tokio::test]
async fn test_gg_from_middle() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("jjgg")
        .run()
        .await;

    // gg should go to first line
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_gg_from_first() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2")
        .with_keys("gg")
        .run()
        .await;

    // Already at first line
    result.assert_cursor(0, 0);
}

// ============================================================================
// G (go to last line)
// ============================================================================

#[tokio::test]
async fn test_big_g_to_last_line() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("G")
        .run()
        .await;

    // G should go to last line (line 3 = y=2)
    result.assert_cursor(0, 2);
}

#[tokio::test]
async fn test_big_g_already_at_last() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2")
        .with_keys("jG")
        .run()
        .await;

    // j then G, should be at last line
    result.assert_cursor(0, 1);
}

// ============================================================================
// 0 (line start)
// ============================================================================

#[tokio::test]
async fn test_zero_to_line_start() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("$0")
        .run()
        .await;

    // $ to end, 0 to start
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_zero_from_middle() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("www0")
        .run()
        .await;

    // w,w,w to move right, 0 to start
    result.assert_cursor(0, 0);
}

// ============================================================================
// $ (line end)
// ============================================================================

#[tokio::test]
async fn test_dollar_to_line_end() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("$")
        .run()
        .await;

    // $ should go to last char (col 4 for "hello")
    result.assert_cursor(4, 0);
}

#[tokio::test]
async fn test_dollar_on_empty_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello\n\nworld")
        .with_keys("j$")
        .run()
        .await;

    // j to empty line, $ should stay at col 0
    result.assert_cursor(0, 1);
}

// ============================================================================
// Count prefix with motions
// ============================================================================

#[tokio::test]
async fn test_count_j() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3\nline 4\nline 5")
        .with_keys("3j")
        .run()
        .await;

    // 3j should move down 3 lines (to line 4, y=3)
    result.assert_cursor(0, 3);
}

#[tokio::test]
async fn test_count_k() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3\nline 4\nline 5")
        .with_keys("G2k")
        .run()
        .await;

    // G to last line (y=4), 2k up 2 (to y=2)
    result.assert_cursor(0, 2);
}

#[tokio::test]
async fn test_count_l() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("5l")
        .run()
        .await;

    // 5l should move right 5 chars (to col 5)
    result.assert_cursor(5, 0);
}

#[tokio::test]
async fn test_count_h() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("$3h")
        .run()
        .await;

    // $ to end (col 10), 3h back 3 (to col 7)
    result.assert_cursor(7, 0);
}

// ============================================================================
// gg and G with count
// ============================================================================

#[tokio::test]
async fn test_count_gg() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3\nline 4\nline 5")
        .with_keys("3gg")
        .run()
        .await;

    // 3gg should go to line 3 (y=2, 0-indexed)
    result.assert_cursor(0, 2);
}

#[tokio::test]
async fn test_count_big_g() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3\nline 4\nline 5")
        .with_keys("2G")
        .run()
        .await;

    // 2G should go to line 2 (y=1, 0-indexed)
    result.assert_cursor(0, 1);
}
