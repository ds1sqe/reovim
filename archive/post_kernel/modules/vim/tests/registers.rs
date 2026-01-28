//! Register tests (unnamed, named, yank types).
//!
//! Tests for register functionality including:
//! - Unnamed register (`"`) population via yy/dd
//! - Yank types (linewise vs characterwise)
//! - Named register selection (`"a`) - requires prefix support

use runner::testing::IntegrationTest;

#[tokio::test]
async fn test_yy_populates_unnamed_register() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("yy")
        .run()
        .await;

    // Check unnamed register via debug/registers
    result.assert_register("\"", "hello\n", "linewise");
}

#[tokio::test]
async fn test_dd_populates_unnamed_register() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("dd")
        .run()
        .await;

    result.assert_register("\"", "hello\n", "linewise");
}

#[tokio::test]
async fn test_yw_char_yank_type() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("yw")
        .run()
        .await;

    result.assert_register("\"", "hello ", "characterwise");
}

#[tokio::test]
async fn test_named_register_a() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("\"ayy")
        .run()
        .await;

    result.assert_register("a", "hello\n", "linewise");
}

/// Test that dd on last line of 2-line buffer leaves single line
#[tokio::test]
async fn test_dd_last_line_of_two() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello\nworld")
        .send_keys("jdd")
        .run()
        .await;

    // After deleting last line, should have just "hello" - no trailing empty line
    result.assert_buffer_eq("hello");
    // Cursor should be at end of remaining line (last valid position)
    result.assert_cursor(0, 4);
}

/// Debug: Test jdd followed by p to isolate paste behavior
#[tokio::test]
async fn test_jdd_then_p() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello\nworld")
        .send_keys("jddp") // Delete world, paste from unnamed register
        .run()
        .await;

    // dd puts "world\n" in unnamed register, p pastes it below
    // Expected: "hello\nworld"
    result.assert_buffer_eq("hello\nworld");
}

#[tokio::test]
async fn test_named_register_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello\nworld")
        .send_keys("\"ayyjdd\"ap")
        .run()
        .await;

    result.assert_buffer_eq("hello\nhello");
}
