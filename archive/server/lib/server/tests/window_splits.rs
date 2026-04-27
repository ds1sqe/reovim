//! Regression tests for window split cursor/viewport behavior.
//!
//! These tests verify that splitting windows preserves cursor state
//! and that navigating between split panes doesn't reset cursor positions.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-server --test window_splits
//! ```
//!
//! # Log Files
//!
//! Test logs are captured to `tmp/test-logs/{test_name}_{timestamp}.log`.

use reovim_testing::IntegrationTest;

// ============================================================================
// Split Cursor Inheritance
// ============================================================================

/// Regression: vertical split should inherit cursor position from source window.
///
/// When splitting a window, the new pane should start at the same cursor
/// position as the source. Without this, the new pane opens at line 0
/// regardless of where the user was editing.
///
/// Root cause: `Window::with_id_and_buffer()` hardcodes `CursorPosition::origin()`
/// instead of copying from the source window.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_vsplit_inherits_cursor_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9")
        .with_cursor_at(5, 0)
        .send_keys("<C-w>v")
        .with_delay(100)
        .send_keys("<C-w>l")
        .with_delay(100)
        .run()
        .await;
    // New (right) pane should inherit cursor at line 5 from source window.
    // Bug: cursor is at line 0 because split creates window with origin().
    result.assert_cursor(5, 0);
}

/// Regression: horizontal split should inherit cursor position from source window.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_hsplit_inherits_cursor_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9")
        .with_cursor_at(5, 0)
        .send_keys("<C-w>s")
        .with_delay(100)
        .send_keys("<C-w>j")
        .with_delay(100)
        .run()
        .await;
    // New (bottom) pane should inherit cursor at line 5 from source window.
    result.assert_cursor(5, 0);
}

/// Regression: split should inherit cursor column, not just line.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_vsplit_inherits_cursor_column() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world\nsecond line")
        .with_cursor_at(0, 6)
        .send_keys("<C-w>v")
        .with_delay(100)
        .send_keys("<C-w>l")
        .with_delay(100)
        .run()
        .await;
    // New pane should inherit both line and column from source window.
    result.assert_cursor(0, 6);
}

// ============================================================================
// Focus Change Cursor Preservation
// ============================================================================

/// Regression: navigating right→left should not reset right pane cursor to line 0.
///
/// User-reported bug: with left/right split, going from right pane to left pane
/// causes the right pane's line to jump to 0.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_focus_left_preserves_right_pane_cursor() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9")
        .send_keys("<C-w>v")
        .with_delay(100)
        .send_keys("<C-w>l")
        .with_delay(100)
        // Move cursor down in right pane
        .send_keys("3j")
        .with_delay(100)
        // Navigate away to left pane and back
        .send_keys("<C-w>h")
        .with_delay(100)
        .send_keys("<C-w>l")
        .with_delay(100)
        .run()
        .await;
    // Right pane cursor should still be at line 3, not reset to 0.
    result.assert_cursor(3, 0);
}

/// Regression: navigating left→right should not reset left pane cursor.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_focus_right_preserves_left_pane_cursor() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9")
        .with_cursor_at(7, 0)
        .send_keys("<C-w>v")
        .with_delay(100)
        // Focus right pane
        .send_keys("<C-w>l")
        .with_delay(100)
        // Go back to left pane
        .send_keys("<C-w>h")
        .with_delay(100)
        .run()
        .await;
    // Left pane cursor should remain at line 7.
    result.assert_cursor(7, 0);
}
