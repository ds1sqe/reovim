//! Undo/redo tests (u, Ctrl-R).

mod common;
use common::IntegrationTest;

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undo_delete_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("ddu")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_redo_after_undo() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("ddu<C-r>")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_multiple_undo() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("dddduu")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 2\nline 3");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undo_insert() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ihello<Esc>u")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_multiple_redo() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("dddduuu<C-r><C-r>")
        .run()
        .await;
    result.assert_buffer_eq("line 3");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undo_change_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("cwXXX<Esc>u")
        .run()
        .await;
    result.assert_buffer_eq("hello world");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undo_nothing_to_undo() {
    // Undo on fresh buffer should not crash
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("u")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_redo_nothing_to_redo() {
    // Redo without prior undo should not crash
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-r>")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}
