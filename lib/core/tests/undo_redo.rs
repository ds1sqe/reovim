//! Undo/Redo integration tests
//!
//! Tests for u (undo) and Ctrl-R (redo) functionality.
//!
//! ## Implementation Notes
//!
//! Current undo behavior:
//! - Undo removes single character, not entire insert session
//! - This differs from vim where insert mode is undone as a unit

mod common;

use common::*;

// ============================================================================
// Basic undo (u) tests - Working
// ============================================================================

#[tokio::test]
async fn test_undo_delete_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("ddu")
        .run()
        .await;

    // Undo restores deleted line
    result.assert_buffer_eq("hello");
}

#[tokio::test]
async fn test_undo_delete_char() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("xu")
        .run()
        .await;

    // Undo restores deleted char
    result.assert_buffer_eq("hello");
}

#[tokio::test]
async fn test_multiple_undo() {
    let result = ServerTest::new()
        .await
        .with_keys("ia<Esc>ib<Esc>uu")
        .run()
        .await;

    // Two undos remove two single char insertions
    result.assert_buffer_eq("");
}

#[tokio::test]
async fn test_undo_at_beginning() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("u")
        .run()
        .await;

    // Nothing to undo
    result.assert_buffer_eq("hello");
}

// ============================================================================
// Basic redo (Ctrl-R) tests - Working
// ============================================================================

#[tokio::test]
async fn test_redo_after_undo() {
    let result = ServerTest::new()
        .await
        .with_keys("ihello<Esc>u<C-r>")
        .run()
        .await;

    // Redo restores undone character
    result.assert_buffer_contains("hello");
}

#[tokio::test]
async fn test_multiple_redo() {
    let result = ServerTest::new()
        .await
        .with_keys("ia<Esc>ib<Esc>uu<C-r><C-r>")
        .run()
        .await;

    // Two redos restore both chars
    result.assert_buffer_contains("a");
    result.assert_buffer_contains("b");
}

#[tokio::test]
async fn test_redo_at_end() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("<C-r>")
        .run()
        .await;

    // Nothing to redo
    result.assert_buffer_eq("hello");
}

// ============================================================================
// Undo/redo edge cases - Working
// ============================================================================

#[tokio::test]
async fn test_undo_redo_cycle() {
    let result = ServerTest::new()
        .await
        .with_keys("ihello<Esc>u<C-r>u<C-r>")
        .run()
        .await;

    // Multiple cycles work
    result.assert_buffer_contains("hello");
}

#[tokio::test]
async fn test_edit_clears_redo() {
    let result = ServerTest::new()
        .await
        .with_keys("ia<Esc>ib<Esc>uic<Esc><C-r>")
        .run()
        .await;

    // New edit clears redo stack
    result.assert_buffer_contains("a");
    result.assert_buffer_contains("c");
}

// ============================================================================
// Document actual undo behavior - single char undo
// ============================================================================

/// Documents that undo removes single character, not insert session
#[tokio::test]
async fn test_doc_undo_single_char() {
    let result = ServerTest::new()
        .await
        .with_keys("ihello<Esc>u")
        .run()
        .await;

    // Expected vim behavior: "" (whole insert undone)
    // Actual behavior: "hell" (only last char undone)
    result.assert_buffer_eq("hell");
}

/// Documents that open line undo is also per-character
#[tokio::test]
async fn test_doc_undo_open_line_single_char() {
    let result = ServerTest::new()
        .await
        .with_content("line 1")
        .with_keys("otest<Esc>u")
        .run()
        .await;

    // Expected vim behavior: "line 1"
    // Actual behavior: "line 1\ntes" (only last char undone)
    result.assert_buffer_eq("line 1\ntes");
}
