//! Cursor visibility integration tests
//!
//! Verifies that cursor Hide/Show escape codes are properly sent
//! during rendering to prevent cursor blinking during screen updates.

mod common;

use common::*;

/// Cursor should be hidden during render and shown afterward
#[tokio::test]
async fn test_cursor_hidden_during_render() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("j") // Move down to trigger render
        .run()
        .await;

    result.assert_cursor_visibility_managed();
}

/// Multiple movements should maintain Hide/Show pattern
#[tokio::test]
async fn test_cursor_visibility_multiple_movements() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3\nline 4")
        .with_keys("jjk") // Move down twice, up once
        .run()
        .await;

    result.assert_cursor_visibility_managed();
}
