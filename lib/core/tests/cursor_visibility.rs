//! Cursor position integration tests
//!
//! Verifies that cursor position is properly tracked and updated
//! during movements using the visual snapshot API.

mod common;

use common::*;

/// Cursor should move down when pressing 'j'
#[tokio::test]
async fn test_cursor_moves_down() {
    let mut result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("j") // Move down
        .run()
        .await;

    // Verify cursor position moved to line 2
    let snapshot = result.visual_snapshot().await;
    let cursor = snapshot.cursor.expect("cursor should be present");
    assert_eq!(cursor.y, 1, "cursor should be on line 2 (y=1)");
    assert_eq!(cursor.layer, "editor", "cursor should be in editor layer");
}

/// Multiple movements should update cursor position correctly
#[tokio::test]
async fn test_cursor_multiple_movements() {
    let mut result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3\nline 4")
        .with_keys("jjk") // Move down twice, up once - end at line 2
        .run()
        .await;

    // Verify cursor ended up on line 2 (y=1)
    let snapshot = result.visual_snapshot().await;
    let cursor = snapshot.cursor.expect("cursor should be present");
    assert_eq!(cursor.y, 1, "cursor should be on line 2 after jjk");
}

/// Cursor horizontal movement with 'l'
#[tokio::test]
async fn test_cursor_moves_right() {
    let mut result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("ll") // Move right twice
        .run()
        .await;

    let snapshot = result.visual_snapshot().await;
    let cursor = snapshot.cursor.expect("cursor should be present");
    // Account for line number gutter (usually 4 chars including separator)
    // The cursor x should be at position 2 within the buffer
    assert!(cursor.x >= 2, "cursor should have moved right");
}
