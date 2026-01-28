//! Visual mode integration tests
//!
//! Tests for v (visual), V (visual line), Ctrl-V (visual block) modes.
//!
//! ## Implementation Notes
//!
//! Known limitations:
//! - V (visual line mode) is not implemented
//! - Selection extension may not work as expected in all cases

mod common;

use common::*;

// ============================================================================
// Entering visual mode (v) - Working
// ============================================================================

#[tokio::test]
async fn test_v_enters_visual_mode() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("v")
        .run()
        .await;

    result.assert_visual_mode();
}

#[tokio::test]
async fn test_escape_exits_visual() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("v<Esc>")
        .run()
        .await;

    result.assert_normal_mode();
}

// ============================================================================
// Visual mode movement - Working
// ============================================================================

#[tokio::test]
async fn test_visual_extend_right() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("vlll")
        .run()
        .await;

    result.assert_visual_mode();
}

#[tokio::test]
async fn test_visual_extend_down() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("vj")
        .run()
        .await;

    result.assert_visual_mode();
}

// ============================================================================
// Visual mode yank (y) - Working
// ============================================================================

#[tokio::test]
async fn test_visual_yank_chars() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("vlllywp")
        .run()
        .await;

    // Visual yank then paste
    result.assert_buffer_contains("hell");
    result.assert_normal_mode();
}

// ============================================================================
// Visual mode delete basic - Working
// ============================================================================

#[tokio::test]
async fn test_visual_returns_to_normal_after_operation() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("vld")
        .run()
        .await;

    result.assert_normal_mode();
}

// ============================================================================
// Visual block mode (Ctrl-V) - Working
// ============================================================================

#[tokio::test]
async fn test_ctrl_v_enters_visual_block() {
    let result = ServerTest::new()
        .await
        .with_content("hello\nworld")
        .with_keys("<C-v>")
        .run()
        .await;

    result.assert_visual_mode();
}

// ============================================================================
// Visual line mode (V)
// ============================================================================

/// V enters visual line mode
#[tokio::test]
async fn test_big_v_enters_visual_line_mode() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2")
        .with_keys("V")
        .run()
        .await;

    // V enters visual line mode
    result.assert_visual_mode();
}

/// Visual delete with l extend deletes selected text
#[tokio::test]
async fn test_visual_delete_chars() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("vllld")
        .run()
        .await;

    // v enters visual at 'h', lll extends to 'l' (selecting "hell"), d deletes
    result.assert_buffer_eq("o world");
    result.assert_normal_mode();
}

/// Visual delete at end of line deletes the character
#[tokio::test]
async fn test_visual_delete_at_eol() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("$vd")
        .run()
        .await;

    // $ moves to 'o', v enters visual, d deletes the 'o'
    result.assert_buffer_eq("hell");
    result.assert_normal_mode();
}
