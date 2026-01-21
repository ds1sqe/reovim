//! Register tests (unnamed, named, yank types).
//!
//! **Status**: All 5 tests require additional features.
//! - 4 tests require `debug/registers` RPC endpoint to return register contents
//! - 1 test requires register selection prefix (`"a`) support
//!
//! Run `cargo test --ignored` to run the remaining ignored tests.

mod common;
use common::IntegrationTest;

#[tokio::test]
#[ignore = "requires debug/registers RPC to return register contents"]
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
#[ignore = "requires debug/registers RPC to return register contents"]
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
#[ignore = "requires debug/registers RPC to return register contents"]
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
#[ignore = "requires register selection prefix (\"a) support"]
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
#[ignore = "requires register selection (\"a) prefix support"]
async fn test_named_register_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello\nworld")
        .send_keys("\"ayyjdd\"ap")
        .run()
        .await;

    result.assert_buffer_eq("hello\nhello");
}
