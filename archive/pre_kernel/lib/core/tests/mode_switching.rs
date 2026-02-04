//! Mode switching integration tests
//!
//! Tests for mode transitions between normal, insert, visual, and command modes
//! using the server-based test harness.

mod common;

use common::*;

// Tests for entering modes

#[tokio::test]
async fn test_insert_mode_via_i() {
    let result = ServerTest::new().await.with_keys("i").run().await;

    result.assert_insert_mode();
}

#[tokio::test]
async fn test_insert_mode_via_a() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("a")
        .run()
        .await;

    result.assert_insert_mode();
}

#[tokio::test]
async fn test_visual_mode() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("v")
        .run()
        .await;

    result.assert_visual_mode();
}

#[tokio::test]
async fn test_command_mode() {
    let result = ServerTest::new().await.with_keys(":").run().await;

    result.assert_command_mode();
}

// Tests for returning to normal mode from other modes

#[tokio::test]
async fn test_normal_to_insert_and_back() {
    let result = ServerTest::new().await.with_keys("i<Esc>").run().await;

    result.assert_normal_mode();
}

#[tokio::test]
async fn test_visual_mode_exit() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("v<Esc>")
        .run()
        .await;

    result.assert_normal_mode();
}

#[tokio::test]
async fn test_command_mode_escape() {
    let result = ServerTest::new().await.with_keys(":<Esc>").run().await;

    result.assert_normal_mode();
}

#[tokio::test]
async fn test_multiple_mode_switches() {
    // i → insert, Esc → normal, v → visual, Esc → normal, : → command, Esc → normal
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("i<Esc>v<Esc>:<Esc>")
        .run()
        .await;

    result.assert_normal_mode();
}
