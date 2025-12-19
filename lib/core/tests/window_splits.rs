//! Window split integration tests
//!
//! Tests for smart window focus/split functionality via <leader>w* keymaps.
//!
//! These tests verify that:
//! 1. Smart focus commands create splits when no adjacent window exists
//! 2. Smart focus commands navigate to existing windows when they exist
//! 3. Explicit split commands (:vs, :sp) work correctly

mod common;

use common::*;

// ============================================================================
// Smart Window Focus/Split tests
// ============================================================================

/// Starting with a single window, <leader>wl should create a vertical split
#[tokio::test]
async fn test_smart_focus_right_creates_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wl") // <leader>wl - smart focus right
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2); // Should have created a split
}

/// Starting with a single window, <leader>wj should create a horizontal split
#[tokio::test]
async fn test_smart_focus_down_creates_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wj") // <leader>wj - smart focus down
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2); // Should have created a split
}

/// Starting with a single window, <leader>wh should create a vertical split
#[tokio::test]
async fn test_smart_focus_left_creates_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wh") // <leader>wh - smart focus left
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2); // Should have created a split
}

/// Starting with a single window, <leader>wk should create a horizontal split
#[tokio::test]
async fn test_smart_focus_up_creates_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wk") // <leader>wk - smart focus up
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2); // Should have created a split
}

/// After creating a split, smart focus should navigate (not create another split)
#[tokio::test]
async fn test_smart_focus_navigates_to_existing_window() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wl") // Create first split (now 2 windows)
        .with_delay(100)
        .with_keys(" wh") // Navigate back left (should NOT create 3rd window)
        .with_delay(100)
        .with_keys(" wl") // Navigate right again (should NOT create 3rd window)
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2); // Should still be 2 windows
}

// ============================================================================
// Explicit split command tests
// ============================================================================

/// :vs command should create a vertical split
#[tokio::test]
async fn test_vs_creates_vertical_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(":vs<CR>")
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2);
}

/// :sp command should create a horizontal split
#[tokio::test]
async fn test_sp_creates_horizontal_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(":sp<CR>")
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2);
}

/// <leader>wv should create a vertical split
#[tokio::test]
async fn test_leader_wv_creates_vertical_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wv") // <leader>wv
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2);
}

/// <leader>ws should create a horizontal split
#[tokio::test]
async fn test_leader_ws_creates_horizontal_split() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" ws") // <leader>ws
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2);
}

// ============================================================================
// Window close tests
// ============================================================================

/// <leader>wc should close the current window
#[tokio::test]
async fn test_leader_wc_closes_window() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wv") // Create split first
        .with_delay(100)
        .with_keys(" wc") // Close current window
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(1); // Back to single window
}

/// Multiple splits should accumulate
#[tokio::test]
async fn test_multiple_splits() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys(" wv") // First vertical split
        .with_delay(100)
        .with_keys(" wv") // Second vertical split
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(3); // Original + 2 splits
}

// ============================================================================
// Shared Buffer Cursor Tests (multiple windows viewing same buffer)
// ============================================================================

/// Test: Two windows sharing same buffer preserve independent cursors
/// After :vs, both windows show same buffer. Moving cursor in one window
/// should not affect the other window's cursor when switching back.
#[tokio::test]
async fn test_shared_buffer_two_windows_cursor_preserved() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Line 1\nLine 2\nLine 3\nLine 4\nLine 5")
        // Win1: move to line 3 (0-indexed: line 2)
        .with_keys("jj")
        .with_delay(200)
        // Create Win2 (vertical split) - same buffer
        .with_keys(":vs<CR>")
        .with_delay(200)
        // Win2 (now active): move to line 5 (0-indexed: line 4)
        .with_keys("jjjj")
        .with_delay(200)
        // Navigate back to Win1
        .with_keys(" wh")
        .with_delay(200)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2);
    let snap = result.visual_snapshot().await;
    let cursor = snap.cursor.expect("Cursor should be present");
    // Win1 cursor should be at line 2 (where we left it with jj)
    assert_eq!(cursor.y, 2, "Win1 cursor should be preserved at line 2, got {}", cursor.y);
}

/// Test: Three windows all sharing same buffer - roundtrip preserves cursor
/// Win1 → Win2 → Win3 → Win2 → Win1 should preserve Win1's cursor position
#[tokio::test]
async fn test_shared_buffer_three_windows_roundtrip() {
    let mut result = ServerTest::new()
        .await
        .with_size(120, 24)
        .with_content("Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7")
        // Win1: move to line 4 (0-indexed: line 3)
        .with_keys("jjj")
        .with_delay(50)
        // Create Win2 (split right)
        .with_keys(":vs<CR>")
        .with_delay(100)
        // Win2: move to line 2 (0-indexed: line 1)
        .with_keys("j")
        .with_delay(50)
        // Create Win3 (split right again)
        .with_keys(":vs<CR>")
        .with_delay(100)
        // Win3: move to line 6 (0-indexed: line 5)
        .with_keys("jjjjj")
        .with_delay(50)
        // Navigate back: Win3 → Win2 → Win1
        .with_keys(" wh")
        .with_delay(100)
        .with_keys(" wh")
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(3);
    let snap = result.visual_snapshot().await;
    let cursor = snap.cursor.expect("Cursor should be present");
    // Win1 cursor should still be at line 3 (where we moved with jjj)
    assert_eq!(
        cursor.y, 3,
        "Win1 cursor should be preserved at line 3 after roundtrip, got {}",
        cursor.y
    );
}
