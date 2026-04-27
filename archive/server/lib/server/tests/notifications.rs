//! E2E tests for notification emission.
//!
//! These tests verify that state changes properly emit notifications
//! to connected clients via gRPC v2 streaming.
//!
//! # Architecture Context
//!
//! From the server/client architecture:
//! - Server emits notifications for state changes
//! - Clients receive notifications via `NotificationService.Subscribe`
//! - This enables the push model (no polling)
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-server --test notifications
//! ```

use std::time::Duration;

use {reovim_client_cli::GrpcClient, reovim_testing::TestServerHarness};

/// Helper to connect with retry.
async fn connect_with_retry(addr: &str) -> GrpcClient {
    let mut attempts = 0;
    loop {
        match GrpcClient::connect(addr).await {
            Ok(c) => return c,
            Err(_) if attempts < 20 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => panic!("Failed to connect after 20 attempts: {e}"),
        }
    }
}

/// Test that server accepts connections.
///
/// This is a basic connectivity test to verify the test infrastructure works.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_server_accepts_connection() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;
    let response = client.ping().await.expect("Ping failed");
    assert!(!response.pong.is_empty());
}

/// Test that mode changes are reflected in state queries.
///
/// `get_mode()` removed in v3; mode state is now queried via `get_projections("text.mode")`.
/// This test is deferred until the integration test helper is updated.
#[tokio::test]
#[ignore = "get_mode removed in v3; mode queried via projections (#753)"]
#[allow(clippy::significant_drop_tightening)]
async fn test_mode_change_reflected_in_state() {}

/// Test that buffer modifications are reflected in content queries.
///
/// `get_buffer_content()` removed in v3; content queried via projections (#753).
/// This test is deferred until the integration test helper is updated.
#[tokio::test]
#[ignore = "get_buffer_content removed in v3; content queried via projections (#753)"]
#[allow(clippy::significant_drop_tightening)]
async fn test_buffer_modification_reflected_in_content() {}

/// Test that cursor position updates are reflected in queries.
///
/// `get_cursor()` removed in v3; cursor state queried via projections (#753).
/// This test is deferred until the integration test helper is updated.
#[tokio::test]
#[ignore = "get_cursor removed in v3; cursor queried via projections (#753)"]
#[allow(clippy::significant_drop_tightening)]
async fn test_cursor_move_reflected_in_position() {}

// ============================================================================
// Notification Streaming Tests (Deferred)
// ============================================================================
//
// The following tests require GrpcClient to support notification streaming.
// They are commented out until that functionality is added.
//
// /// Test that mode changes emit notifications.
// #[tokio::test]
// #[ignore = "Requires notification streaming support"]
// async fn test_mode_change_emits_notification() {
//     // ... would use client.subscribe(["mode_changed"]) ...
// }
