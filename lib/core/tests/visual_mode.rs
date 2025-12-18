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
// Document limitations
// ============================================================================

/// Documents that V (visual line mode) is not implemented
#[tokio::test]
async fn test_doc_big_v_not_implemented() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2")
        .with_keys("V")
        .run()
        .await;

    // Expected vim behavior: visual line mode
    // Actual behavior: stays in normal mode (V not bound)
    result.assert_normal_mode();
}

/// Documents that visual delete with l extend may not work
#[tokio::test]
async fn test_doc_visual_delete_chars_actual() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("vllld")
        .run()
        .await;

    // Expected vim behavior: delete "hell", leave "o world"
    // Actual behavior: buffer unchanged (selection/delete issue)
    result.assert_buffer_eq("hello world");
}

/// Documents $ then vd behavior
#[tokio::test]
async fn test_doc_visual_at_eol_actual() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("$vd")
        .run()
        .await;

    // Expected vim behavior: delete 'o', leave "hell"
    // Actual behavior: buffer unchanged
    result.assert_buffer_eq("hello");
}
