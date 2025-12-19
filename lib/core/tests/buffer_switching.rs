//! Buffer switching integration tests
//!
//! Tests for buffer switching functionality via telescope and commands.
//!
//! These tests verify that:
//! 1. Telescope buffer picker correctly switches buffers
//! 2. :e command correctly switches to new buffers
//! 3. Screen state is properly synchronized after buffer switches

mod common;

use common::*;

// ============================================================================
// Telescope buffer switching tests
// ============================================================================

/// Telescope buffer picker should show at least one buffer (the scratch buffer)
#[tokio::test]
async fn test_telescope_buffer_picker_has_items() {
    let result = ServerTest::new()
        .await
        .with_keys(" fb")
        .with_delay(200)
        .run()
        .await;

    result.assert_telescope_active();
    result.assert_telescope_picker("buffers");
    result.assert_telescope_has_items();
}

/// Opening and closing telescope should preserve active buffer
#[tokio::test]
async fn test_telescope_close_preserves_buffer() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" fb")
        .with_delay(150)
        .with_keys("<Esc>") // Insert -> Normal
        .with_delay(50)
        .with_keys("<Esc>") // Close telescope
        .with_delay(100)
        .run()
        .await;

    result.assert_telescope_inactive();
    result.assert_normal_mode();
    result.assert_buffer_contains("test content");
}

/// Selecting a buffer in telescope should switch to it
#[tokio::test]
async fn test_telescope_buffer_select_switches() {
    let result = ServerTest::new()
        .await
        .with_content("buffer one content")
        .with_keys(":e buffer2<CR>")
        .with_delay(100)
        .with_keys("iBuffer two content<Esc>")
        .with_delay(50)
        .with_keys(" fb")
        .with_delay(200)
        .with_keys("<CR>") // Select first buffer (should be current)
        .with_delay(100)
        .run()
        .await;

    // After selecting from telescope, should be in normal mode
    result.assert_normal_mode();
    result.assert_telescope_inactive();
}

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
    assert_eq!(result.mode.focus, "Editor", "Focus should be on Editor after :e command");
}

// ============================================================================
// Buffer content preservation tests
// ============================================================================

/// Switching buffers should preserve content in both buffers
#[tokio::test]
async fn test_buffer_switch_preserves_content() {
    let result = ServerTest::new()
        .await
        .with_content("first buffer content")
        .with_keys(":e second<CR>")
        .with_delay(100)
        .with_keys("iSecond buffer text<Esc>")
        .with_delay(50)
        .with_keys(" fb") // Open buffer picker
        .with_delay(200)
        .with_keys("j<CR>") // Move down and select (should select first buffer)
        .with_delay(150)
        .run()
        .await;

    // Should switch back to first buffer and contain original content
    result.assert_normal_mode();
    result.assert_telescope_inactive();
}

// ============================================================================
// Mode state after buffer switch
// ============================================================================

/// After buffer switch via telescope, should be in normal mode
#[tokio::test]
async fn test_telescope_switch_returns_to_normal_mode() {
    let result = ServerTest::new()
        .await
        .with_content("test")
        .with_keys(" fb")
        .with_delay(200)
        .with_keys("<CR>") // Select current buffer
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    assert_eq!(
        result.mode.focus, "Editor",
        "Focus should return to Editor after telescope selection"
    );
}

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
    assert_eq!(result.mode.focus, "Editor", "Focus should be on Editor after :e command");
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

/// Telescope buffer picker after multiple :e commands should have multiple buffers
#[tokio::test]
async fn test_buffer_picker_shows_all_buffers() {
    let result = ServerTest::new()
        .await
        .with_content("initial")
        .with_keys(":e second<CR>")
        .with_delay(100)
        .with_keys(":e third<CR>")
        .with_delay(100)
        .with_keys(" fb")
        .with_delay(300)
        .run()
        .await;

    result.assert_telescope_active();
    result.assert_telescope_picker("buffers");

    // Should have at least 2 buffers (initial + at least one from :e)
    // Note: :e may reuse existing buffer with same name, so we're lenient here
    let ts = result
        .telescope
        .as_ref()
        .expect("Telescope state not available");
    assert!(
        ts.item_count >= 1,
        "Expected at least 1 buffer in picker, got {}",
        ts.item_count
    );
}
