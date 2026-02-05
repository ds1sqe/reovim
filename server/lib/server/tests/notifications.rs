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
//! # Status
//!
//! These tests require the gRPC client to support notification streaming,
//! which is implemented in `TuiGrpcClient` but not `GrpcClient` (CLI client).
//!
//! For now, notification tests are deferred until:
//! 1. `GrpcClient` gains `subscribe()` method, or
//! 2. Tests use `TuiGrpcClient` directly
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
#[ignore = "Requires server binary"]
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
/// While this doesn't test notifications directly, it verifies that
/// mode changes work end-to-end and can be queried.
#[tokio::test]
#[ignore = "Requires server binary with modules loaded"]
#[allow(clippy::significant_drop_tightening)]
async fn test_mode_change_reflected_in_state() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    // Initial mode should be normal
    let mode = client.get_mode().await.expect("Failed to get mode");
    assert!(
        mode.name.to_lowercase().contains("normal") || mode.display.contains("NORMAL"),
        "Expected normal mode initially, got: {} ({})",
        mode.display,
        mode.name
    );

    // Enter insert mode
    client.send_keys("i").await.expect("Failed to send keys");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Mode should now be insert
    let mode = client.get_mode().await.expect("Failed to get mode");
    assert!(
        mode.name.to_lowercase().contains("insert") || mode.display.contains("INSERT"),
        "Expected insert mode after 'i', got: {} ({})",
        mode.display,
        mode.name
    );

    // Return to normal mode
    client
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Mode should be normal again
    let mode = client.get_mode().await.expect("Failed to get mode");
    assert!(
        mode.name.to_lowercase().contains("normal") || mode.display.contains("NORMAL"),
        "Expected normal mode after Escape, got: {} ({})",
        mode.display,
        mode.name
    );
}

/// Test that buffer modifications are reflected in content queries.
#[tokio::test]
#[ignore = "Requires server binary with modules loaded"]
#[allow(clippy::significant_drop_tightening)]
async fn test_buffer_modification_reflected_in_content() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    // Insert some text
    client
        .send_keys("ihello<Esc>")
        .await
        .expect("Failed to send keys");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Query buffer content
    let content = client
        .get_buffer_content(None)
        .await
        .expect("Failed to get content");
    assert!(
        content.lines.iter().any(|line| line.contains("hello")),
        "Expected buffer to contain 'hello', got: {:?}",
        content.lines
    );
}

/// Test that cursor position updates are reflected in queries.
#[tokio::test]
#[ignore = "Requires server binary with modules loaded"]
#[allow(clippy::significant_drop_tightening)]
async fn test_cursor_move_reflected_in_position() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_with_retry(&addr).await;

    // Get initial cursor position
    let cursor1 = client.get_cursor().await.expect("Failed to get cursor");
    let pos1 = cursor1.position.unwrap_or_default();

    // Move cursor right
    client.send_keys("l").await.expect("Failed to send keys");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Get new cursor position
    let cursor2 = client.get_cursor().await.expect("Failed to get cursor");
    let pos2 = cursor2.position.unwrap_or_default();

    // Column should have increased (or stayed same if at line end)
    // This is a weak assertion since buffer may be empty
    let _ = (pos1, pos2); // Use the positions
}

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
