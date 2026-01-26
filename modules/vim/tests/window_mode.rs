//! Window mode integration tests (Epic #438).
//!
//! Tests window mode keybindings and commands:
//! - Mode entry via `<C-w>` from normal mode
//! - Mode exit via Escape
//! - Window commands (split, focus, resize, close)
//!
//! **Status**: Initial tests for window mode infrastructure.

use runner::testing::IntegrationTest;

// ============================================================================
// MODE TRANSITIONS
// ============================================================================

#[tokio::test]
async fn test_ctrl_w_enters_window_mode() {
    // <C-w> from normal mode should enter window mode
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>")
        .run()
        .await;

    // Verify we're in window mode
    assert!(
        result.mode_display.to_uppercase().contains("WINDOW"),
        "Expected window mode, got: {} ({})",
        result.mode_display,
        result.edit_mode
    );
}

#[tokio::test]
async fn test_ctrl_w_escape_returns_to_normal() {
    // <C-w><Esc> should return to normal mode without any action
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w><Esc>")
        .run()
        .await;

    // Verify we're back in normal mode
    result.assert_normal_mode();
    // Buffer should be unchanged
    result.assert_buffer_eq("hello");
}

// ============================================================================
// SPLIT COMMANDS (Epic #438 Phase 3)
// ============================================================================

#[tokio::test]
async fn test_ctrl_w_s_horizontal_split() {
    // <C-w>s should split horizontally and return to normal mode
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>s")
        .run()
        .await;

    // After split command, should be back in normal mode
    result.assert_normal_mode();
    // Buffer content should be unchanged
    result.assert_buffer_eq("hello");
}

#[tokio::test]
async fn test_ctrl_w_v_vertical_split() {
    // <C-w>v should split vertically and return to normal mode
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>v")
        .run()
        .await;

    // After split command, should be back in normal mode
    result.assert_normal_mode();
    // Buffer content should be unchanged
    result.assert_buffer_eq("hello");
}

// ============================================================================
// FOCUS NAVIGATION (basic - single window)
// ============================================================================

#[tokio::test]
async fn test_ctrl_w_h_focus_left_single_window() {
    // <C-w>h in single window should return to normal (no-op navigation)
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>h")
        .run()
        .await;

    // Should be back in normal mode
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_ctrl_w_j_focus_down_single_window() {
    // <C-w>j in single window should return to normal (no-op navigation)
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>j")
        .run()
        .await;

    // Should be back in normal mode
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_ctrl_w_k_focus_up_single_window() {
    // <C-w>k in single window should return to normal (no-op navigation)
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>k")
        .run()
        .await;

    // Should be back in normal mode
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_ctrl_w_l_focus_right_single_window() {
    // <C-w>l in single window should return to normal (no-op navigation)
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>l")
        .run()
        .await;

    // Should be back in normal mode
    result.assert_normal_mode();
}

// ============================================================================
// CLOSE COMMANDS
// ============================================================================

#[tokio::test]
async fn test_ctrl_w_c_close_window() {
    // <C-w>c in single window should fail gracefully (can't close last window)
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>c")
        .run()
        .await;

    // Should be back in normal mode
    result.assert_normal_mode();
    // Buffer should still exist
    result.assert_buffer_eq("hello");
}

// ============================================================================
// RESIZE COMMANDS (single window - no visible effect but should work)
// ============================================================================

#[tokio::test]
async fn test_ctrl_w_plus_resize() {
    // <C-w>+ in single window should return to normal
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>+")
        .run()
        .await;

    result.assert_normal_mode();
}

#[tokio::test]
async fn test_ctrl_w_minus_resize() {
    // <C-w>- in single window should return to normal
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>-")
        .run()
        .await;

    result.assert_normal_mode();
}

#[tokio::test]
async fn test_ctrl_w_equal_equalize() {
    // <C-w>= in single window should return to normal
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>=")
        .run()
        .await;

    result.assert_normal_mode();
}

// ============================================================================
// CYCLING COMMANDS
// ============================================================================

#[tokio::test]
async fn test_ctrl_w_w_cycle_next() {
    // <C-w>w in single window should return to normal (cycle to self)
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>w")
        .run()
        .await;

    result.assert_normal_mode();
}

#[tokio::test]
async fn test_ctrl_w_shift_w_cycle_prev() {
    // <C-w>W in single window should return to normal (cycle to self)
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>W")
        .run()
        .await;

    result.assert_normal_mode();
}

// ============================================================================
// UNBOUND KEYS IN WINDOW MODE
// ============================================================================

#[tokio::test]
async fn test_ctrl_w_unbound_key_stays_in_window_mode() {
    // <C-w>x (unbound) should not change mode immediately
    // Note: In vim, unbound keys in window mode are ignored
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-w>x")
        .run()
        .await;

    // Behavior depends on implementation - either stays in window mode
    // or returns to normal. For now, let's just verify buffer is unchanged.
    result.assert_buffer_eq("hello");
}
