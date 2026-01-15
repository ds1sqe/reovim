//! Edge case tests (empty buffer, Unicode, boundaries).

mod common;
use common::IntegrationTest;

// ============================================================================
// EMPTY BUFFER
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_dd_empty_buffer() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("dd")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_x_empty_buffer() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("x")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_yy_empty_buffer() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("yy")
        .run()
        .await;
    result.assert_buffer_eq("");
}

// ============================================================================
// UNICODE
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_dd_emoji_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello\n😀😁😂\nworld")
        .send_keys("jdd")
        .run()
        .await;
    result.assert_buffer_eq("hello\nworld");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_yy_cjk_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("你好世界")
        .send_keys("yyp")
        .run()
        .await;
    result.assert_buffer_eq("你好世界\n你好世界");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_insert_unicode() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("i🎉<Esc>")
        .run()
        .await;
    result.assert_buffer_contains("🎉");
}

// ============================================================================
// SINGLE CHARACTER
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_dd_single_char() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("a")
        .send_keys("dd")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_x_single_char() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("a")
        .send_keys("x")
        .run()
        .await;
    result.assert_buffer_eq("");
}

// ============================================================================
// SPECIAL CHARACTERS
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_dd_line_with_tabs() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("\thello\tworld")
        .send_keys("dd")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_yy_line_with_trailing_spaces() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello   \nworld")
        .send_keys("yyp")
        .run()
        .await;
    result.assert_buffer_eq("hello   \nhello   \nworld");
}
