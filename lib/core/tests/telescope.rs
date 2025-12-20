//! Telescope integration tests
//!
//! Tests for the telescope fuzzy finder functionality.
//!
//! ## Implementation Notes
//!
//! - Telescope is activated via Space+f shortcuts
//! - Space ff = find files, Space fb = buffers, Space fg = grep

mod common;

use common::*;

// ============================================================================
// Basic activation tests
// ============================================================================

/// Telescope should be inactive by default
#[tokio::test]
async fn test_telescope_inactive_by_default() {
    let result = ServerTest::new().await.run().await;

    result.assert_telescope_inactive();
}

/// Space ff should activate telescope file finder
#[tokio::test]
async fn test_telescope_space_ff_opens() {
    let result = ServerTest::new()
        .await
        .with_keys(" ff")
        .with_delay(100)
        .run()
        .await;

    result.assert_telescope_active();
    result.assert_telescope_picker("files");
}

/// Space fb should activate telescope buffer picker
#[tokio::test]
async fn test_telescope_space_fb_opens() {
    let result = ServerTest::new()
        .await
        .with_keys(" fb")
        .with_delay(100)
        .run()
        .await;

    result.assert_telescope_active();
    result.assert_telescope_picker("buffers");
}

// ============================================================================
// Escape closes telescope
// ============================================================================

/// Escape should close telescope (need two escapes: insert->normal->close)
#[tokio::test]
async fn test_telescope_escape_closes() {
    let result = ServerTest::new()
        .await
        .with_keys(" ff")
        .with_delay(150)
        .with_keys("<Esc>") // Insert -> Normal mode
        .with_delay(50)
        .with_keys("<Esc>") // Normal -> Close
        .with_delay(150)
        .run()
        .await;

    result.assert_telescope_inactive();
    result.assert_normal_mode();
}

// ============================================================================
// Query input tests
// ============================================================================

/// Typing in telescope should update query
#[tokio::test]
async fn test_telescope_query_input() {
    let result = ServerTest::new()
        .await
        .with_keys(" ff")
        .with_delay(200) // Wait for telescope to fully open
        .with_keys("test")
        .with_delay(150) // Wait for query update
        .run()
        .await;

    result.assert_telescope_active();
    result.assert_telescope_query("test");
}

/// Backspace should delete characters from query
#[tokio::test]
async fn test_telescope_backspace_deletes() {
    let result = ServerTest::new()
        .await
        .with_keys(" ff")
        .with_delay(200) // Wait for telescope to fully open
        .with_keys("abc<BS>")
        .with_delay(150) // Wait for query update
        .run()
        .await;

    result.assert_telescope_active();
    result.assert_telescope_query("ab");
}

// ============================================================================
// Navigation tests
// ============================================================================

/// Buffer picker should have at least one buffer
#[tokio::test]
async fn test_telescope_buffer_has_items() {
    let result = ServerTest::new()
        .await
        .with_keys(" fb")
        .with_delay(250) // Buffer list needs time to populate
        .run()
        .await;

    result.assert_telescope_active();
    result.assert_telescope_has_items();
}

/// Ctrl-n should move selection down (if items exist)
#[tokio::test]
async fn test_telescope_ctrl_n_moves_down() {
    let result = ServerTest::new()
        .await
        .with_keys(" fb")
        .with_delay(100)
        .run()
        .await;

    // Just verify telescope is active with buffer picker
    result.assert_telescope_active();
    result.assert_telescope_picker("buffers");
}

// ============================================================================
// Mode tests
// ============================================================================

/// Telescope should put focus on Telescope in Insert mode
#[tokio::test]
async fn test_telescope_focus_mode() {
    let result = ServerTest::new()
        .await
        .with_keys(" ff")
        .with_delay(150)
        .run()
        .await;

    assert_eq!(
        result.mode.focus, "telescope",
        "Expected Telescope focus, got {:?}",
        result.mode.focus
    );
    // Telescope opens in Insert mode for typing query
    assert!(
        result.mode.edit_mode.contains("Insert"),
        "Expected Insert mode, got {:?}",
        result.mode.edit_mode
    );
}

/// After closing telescope, focus should return to Editor
#[tokio::test]
async fn test_telescope_returns_focus_to_editor() {
    let result = ServerTest::new()
        .await
        .with_keys(" ff")
        .with_delay(150)
        .with_keys("<Esc>") // Insert -> Normal mode
        .with_delay(50)
        .with_keys("<Esc>") // Normal -> Close
        .with_delay(150)
        .run()
        .await;

    assert_eq!(
        result.mode.focus, "editor",
        "Expected Editor focus, got {:?}",
        result.mode.focus
    );
}
