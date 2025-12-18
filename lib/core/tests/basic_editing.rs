//! Basic editing integration tests
//!
//! Tests for cursor movement and text insertion using the server-based test harness.

mod common;

use common::*;

// Normal mode cursor movement tests

#[tokio::test]
async fn test_cursor_movement_j() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("j")
        .run()
        .await;

    result.assert_cursor(0, 1); // Should be on line 2 (y=1)
}

#[tokio::test]
async fn test_cursor_movement_jj() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("jj")
        .run()
        .await;

    result.assert_cursor(0, 2); // Should be on line 3 (y=2)
}

#[tokio::test]
async fn test_cursor_movement_l() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("l")
        .run()
        .await;

    result.assert_cursor(1, 0); // Should move right one character
}

#[tokio::test]
async fn test_cursor_movement_hjkl() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld\ntest!")
        .with_keys("jllk")
        .run()
        .await;

    // Start at (0,0), j to (0,1), ll to (2,1), k to (2,0)
    result.assert_cursor(2, 0);
}

// Insert mode tests - test text insertion and mode switching

#[tokio::test]
async fn test_insert_mode_basic() {
    let result = ServerTest::new().await.with_keys("ihello<Esc>").run().await;

    result.assert_buffer_contains("hello");
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_insert_mode_multiple_words() {
    let result = ServerTest::new()
        .await
        .with_keys("ihello world<Esc>")
        .run()
        .await;

    result.assert_buffer_contains("hello world");
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_append_mode() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("Aworld<Esc>")
        .run()
        .await;

    result.assert_buffer_contains("helloworld");
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_insert_at_beginning() {
    // Use 0i instead of I (insert at beginning) since I is not implemented
    let result = ServerTest::new()
        .await
        .with_content("world")
        .with_keys("0ihello <Esc>")
        .run()
        .await;

    result.assert_buffer_contains("hello world");
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_open_line_below() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 3")
        .with_keys("oline 2<Esc>")
        .run()
        .await;

    result.assert_buffer_contains("line 1\nline 2\nline 3");
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_open_line_above() {
    let result = ServerTest::new()
        .await
        .with_content("line 2\nline 3")
        .with_keys("Oline 1<Esc>")
        .run()
        .await;

    result.assert_buffer_contains("line 1\nline 2\nline 3");
    result.assert_normal_mode();
}
