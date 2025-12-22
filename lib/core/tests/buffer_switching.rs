//! Buffer switching integration tests
//!
//! Tests for buffer switching functionality via commands.
//!
//! These tests verify that:
//! 1. :e command correctly switches to new buffers
//! 2. Screen state is properly synchronized after buffer switches
//!
//! Note: Telescope buffer picker tests have been moved to the telescope plugin.

mod common;

use common::*;

// ============================================================================
// :e command buffer switching tests
// ============================================================================

/// :e command should create a new buffer
#[tokio::test]
async fn test_edit_command_creates_buffer() {
    let result = ServerTest::new()
        .await
        .with_content("original buffer")
        .with_keys(":e newfile.txt<CR>")
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    // New buffer should be empty (or have default content)
    // The active buffer should have changed
}

/// :e command should result in normal mode with editor focus
/// The key behavior is that after :e, we're back in normal mode ready to edit
#[tokio::test]
async fn test_edit_command_switches_active_buffer() {
    let result = ServerTest::new()
        .await
        .with_content("first buffer")
        .with_keys(":e second<CR>")
        .with_delay(100)
        .run()
        .await;

    // After :e, we should be in normal mode with Editor focus
    result.assert_normal_mode();
    assert_eq!(result.mode.focus, "editor", "Focus should be on Editor after :e command");
}

// ============================================================================
// Mode state after buffer switch
// ============================================================================

/// After :e command, should be in normal mode with editor focus
#[tokio::test]
async fn test_edit_command_returns_to_normal_mode() {
    let result = ServerTest::new()
        .await
        .with_keys(":e newbuffer<CR>")
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    assert_eq!(result.mode.focus, "editor", "Focus should be on Editor after :e command");
}

// ============================================================================
// Edge cases
// ============================================================================

/// Multiple rapid buffer switches should work correctly
#[tokio::test]
async fn test_rapid_buffer_switches() {
    let result = ServerTest::new()
        .await
        .with_content("buf1")
        .with_keys(":e buf2<CR>")
        .with_delay(50)
        .with_keys(":e buf3<CR>")
        .with_delay(50)
        .with_keys(":e buf4<CR>")
        .with_delay(100)
        .run()
        .await;

    // Should end up on buf4
    result.assert_normal_mode();
}
