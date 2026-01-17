//! Register tests (unnamed, named, yank types).
//!
//! **Status**: 1 test enabled, 4 tests require additional features
//! (register content query API).
//!
//! Run `cargo test --ignored` to run the remaining ignored tests.

mod common;
use common::IntegrationTest;

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_yy_populates_unnamed_register() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("yy")
        .run()
        .await;

    // Check unnamed register via debug/registers
    result.assert_register("\"", "hello\n", "line");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_dd_populates_unnamed_register() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("dd")
        .run()
        .await;

    result.assert_register("\"", "hello\n", "line");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_yw_char_yank_type() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("yw")
        .run()
        .await;

    result.assert_register("\"", "hello ", "char");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_named_register_a() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("\"ayy")
        .run()
        .await;

    result.assert_register("a", "hello\n", "line");
}

#[tokio::test]
async fn test_named_register_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello\nworld")
        .send_keys("\"ayyjdd\"ap")
        .run()
        .await;

    result.assert_buffer_eq("hello\nhello");
}
