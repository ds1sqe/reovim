//! Command mode integration tests
//!
//! Tests for : (command mode) and ex commands.

mod common;

use common::*;

// ============================================================================
// Entering and exiting command mode
// ============================================================================

#[tokio::test]
async fn test_colon_enters_command_mode() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":")
        .run()
        .await;

    result.assert_command_mode();
}

#[tokio::test]
async fn test_escape_exits_command_mode() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":<Esc>")
        .run()
        .await;

    result.assert_normal_mode();
}

// ============================================================================
// Command line editing
// ============================================================================

#[tokio::test]
async fn test_command_line_backspace() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":abc<BS><Esc>")
        .run()
        .await;

    // Backspace removes last char, then escape exits
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_command_line_multiple_backspace() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":test<BS><BS><BS><BS><Esc>")
        .run()
        .await;

    // All backspaces, then escape
    result.assert_normal_mode();
}

// ============================================================================
// :set commands
// ============================================================================

#[tokio::test]
async fn test_set_number() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":set number<CR>")
        .run()
        .await;

    // After :set number, should be back in normal mode
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_set_nonumber() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":set nonumber<CR>")
        .run()
        .await;

    result.assert_normal_mode();
}

#[tokio::test]
async fn test_set_relativenumber() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":set relativenumber<CR>")
        .run()
        .await;

    result.assert_normal_mode();
}

// ============================================================================
// Buffer state after command
// ============================================================================

#[tokio::test]
async fn test_command_preserves_buffer() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys(":set number<CR>")
        .run()
        .await;

    // Buffer should be unchanged after :set
    result.assert_buffer_eq("hello world");
}

// ============================================================================
// Edge cases
// ============================================================================

#[tokio::test]
async fn test_empty_command_enter() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys(":<CR>")
        .run()
        .await;

    // Empty command should just return to normal
    result.assert_normal_mode();
}

/// Documents that : from visual mode doesn't enter command mode
#[tokio::test]
async fn test_doc_command_from_visual_not_working() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("v:")
        .run()
        .await;

    // Expected vim behavior: command mode
    // Actual behavior: stays in visual mode (: not bound in visual mode)
    result.assert_visual_mode();
}
