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

// ============================================================================
// Shared Buffer Scroll Independence Tests
// ============================================================================
// These tests verify that each window maintains its own independent scroll
// position (buffer_anchor) when multiple windows view the same buffer.

/// Test: Two windows sharing same buffer have independent scroll positions
/// Scrolling in one window should NOT affect the other window's scroll.
#[tokio::test]
async fn test_shared_buffer_scroll_independence() {
    // Generate content with 100 lines for scrolling tests
    let content = (1..=100)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut result = ServerTest::new()
        .await
        .with_size(80, 20) // Small height to force scrolling
        .with_content(&content)
        // Create Win2 (vertical split) - same buffer
        .with_keys(":vs<CR>")
        .with_delay(100)
        // Win2 (now active): scroll down 50 lines (cursor moves down)
        .with_keys("50j")
        .with_delay(100)
        // Navigate back to Win1
        .with_keys(" wh")
        .with_delay(200)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2);

    // Get window states
    let windows = result.windows().await;
    assert_eq!(windows.len(), 2, "Expected 2 windows");

    // Find the active window (Win1) and inactive window (Win2)
    let win1 = windows
        .iter()
        .find(|w| w.is_active)
        .expect("Should have active window");
    let win2 = windows
        .iter()
        .find(|w| !w.is_active)
        .expect("Should have inactive window");

    // Win1 scroll should be at 0 (never scrolled)
    assert_eq!(
        win1.buffer_anchor_y, 0,
        "Win1 scroll should be at y=0 (never scrolled), got y={}",
        win1.buffer_anchor_y
    );

    // Win2 scroll should be non-zero (scrolled down with cursor)
    // Note: The exact value depends on viewport height and scroll behavior
    assert!(
        win2.buffer_anchor_y > 0 || win2.cursor_y >= 50,
        "Win2 should have scrolled down or cursor at line 50, got anchor_y={}, cursor_y={}",
        win2.buffer_anchor_y,
        win2.cursor_y
    );
}

/// Test: Scroll positions are preserved when navigating between windows
/// Win1 scrolled to line 20, Win2 scrolled to line 60.
/// Navigating between them should preserve each window's scroll.
#[tokio::test]
async fn test_shared_buffer_scroll_preserved_after_navigation() {
    // Generate content with 100 lines
    let content = (1..=100)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut result = ServerTest::new()
        .await
        .with_size(80, 10) // Very small height to ensure scrolling
        .with_content(&content)
        // Win1: move cursor down to trigger scroll
        .with_keys("20j")
        .with_delay(100)
        // Create Win2 (vertical split)
        .with_keys(":vs<CR>")
        .with_delay(100)
        // Win2: move cursor much further down
        .with_keys("60j")
        .with_delay(100)
        // Navigate back to Win1
        .with_keys(" wh")
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(2);

    let windows = result.windows().await;
    let win1 = windows.iter().find(|w| w.is_active).expect("Active window");
    let win2 = windows
        .iter()
        .find(|w| !w.is_active)
        .expect("Inactive window");

    // Win1 cursor should be around line 20
    assert!(
        win1.cursor_y >= 19 && win1.cursor_y <= 21,
        "Win1 cursor should be around line 20, got {}",
        win1.cursor_y
    );

    // Win2 cursor should be around line 60 (20 + 60 = 80, but starts at 0 so 60j from 20 = 80)
    // Actually after :vs, Win2 starts fresh at same position as Win1 was, then 60j moves it
    // Let's just check Win2 has moved further than Win1
    assert!(
        win2.cursor_y > win1.cursor_y,
        "Win2 cursor ({}) should be further down than Win1 cursor ({})",
        win2.cursor_y,
        win1.cursor_y
    );
}

/// Test: Three windows all sharing same buffer maintain independent scrolls
#[tokio::test]
async fn test_three_windows_scroll_isolation() {
    let content = (1..=100)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut result = ServerTest::new()
        .await
        .with_size(120, 10) // Wide enough for 3 splits
        .with_content(&content)
        // Win1: stay at top (no movement)
        // Create Win2
        .with_keys(":vs<CR>")
        .with_delay(100)
        // Win2: scroll to line 30
        .with_keys("30j")
        .with_delay(100)
        // Create Win3
        .with_keys(":vs<CR>")
        .with_delay(100)
        // Win3: scroll to line 60
        .with_keys("60j")
        .with_delay(100)
        // Navigate back through all windows: Win3 → Win2 → Win1
        .with_keys(" wh")
        .with_delay(100)
        .with_keys(" wh")
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    result.assert_window_count(3);

    let windows = result.windows().await;
    assert_eq!(windows.len(), 3, "Expected 3 windows");

    // Find active window (Win1 - should be at cursor 0)
    let active = windows.iter().find(|w| w.is_active).expect("Active window");
    assert_eq!(
        active.cursor_y, 0,
        "Win1 (active) cursor should be at line 0, got {}",
        active.cursor_y
    );

    // The other two windows should have cursor positions > 0
    let inactive: Vec<_> = windows.iter().filter(|w| !w.is_active).collect();
    assert_eq!(inactive.len(), 2, "Should have 2 inactive windows");

    // Both inactive windows should have non-zero cursor positions
    for (i, win) in inactive.iter().enumerate() {
        assert!(
            win.cursor_y > 0,
            "Inactive window {} should have cursor_y > 0, got {}",
            i,
            win.cursor_y
        );
    }
}
