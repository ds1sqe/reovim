//! Mode switching integration tests
//!
//! Tests for mode transitions between normal, insert, visual, and command modes.
//! Note: Tests involving Escape to return to normal mode have timing issues and are marked ignored.

mod common;

use common::*;

// Tests that work reliably - entering modes

#[tokio::test]
async fn test_insert_mode_via_i() {
    let rt = standard_runtime()
        .with_keys(keys_from_str("i"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    result.assert_insert_mode();
}

#[tokio::test]
async fn test_insert_mode_via_a() {
    let rt = runtime_with_content("hello")
        .with_keys(keys_from_str("a"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    result.assert_insert_mode();
}

#[tokio::test]
async fn test_visual_mode() {
    let rt = runtime_with_content("hello world")
        .with_keys(keys_from_str("v"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    assert!(result.mode.is_visual(), "Expected visual mode, got {:?}", result.mode);
}

#[tokio::test]
async fn test_command_mode() {
    let rt = standard_runtime()
        .with_keys(keys_from_str(":"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    assert!(result.mode.is_command(), "Expected command mode, got {:?}", result.mode);
}

#[tokio::test]
async fn test_quit_command() {
    let rt = standard_runtime()
        .with_keys(keys_from_str(":q<CR>"))
        .build();

    let result = rt.run().await;

    // Should exit cleanly without timeout
    result.assert_no_timeout();
}

// Tests for returning to normal mode from other modes

#[tokio::test]
async fn test_normal_to_insert_and_back() {
    let rt = standard_runtime()
        .with_keys(keys_from_str("i<Esc>"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_visual_mode_exit() {
    let rt = runtime_with_content("hello world")
        .with_keys(keys_from_str("v<Esc>"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_command_mode_escape() {
    let rt = standard_runtime()
        .with_keys(keys_from_str(":<Esc>"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    result.assert_normal_mode();
}

#[tokio::test]
async fn test_multiple_mode_switches() {
    // i → insert, Esc → normal, v → visual, Esc → normal, : → command, Esc → normal
    let rt = runtime_with_content("hello")
        .with_keys(keys_from_str("i<Esc>v<Esc>:<Esc>"))
        .build();

    let result = rt.run().await;

    result.assert_no_timeout();
    result.assert_normal_mode();
}
