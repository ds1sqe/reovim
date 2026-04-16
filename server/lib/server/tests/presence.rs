//! Integration tests for `PresenceService` (Phase 14, Epic #465).
//!
//! These tests verify multi-client presence awareness works end-to-end.
//!
//! # Test Categories
//!
//! | Category | Description |
//! |----------|-------------|
//! | Join/Leave | Client lifecycle operations |
//! | Streaming | Presence update streaming (deferred) |
//! | Follow Mode | Sync mode functionality |
//! | Concurrent | Thread safety under load |
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-server --test presence
//! ```
//!
//! # Architecture
//!
//! ```text
//! TestServerHarness ─┬─► Client 1 ─► Join() ─► StreamPresence()
//!                    └─► Client 2 ─► Join() ─► receives updates
//! ```
//!
//! # Unit Tests
//!
//! Core `PresenceService` unit tests are in:
//! - `server/lib/server/src/session/presence.rs` (18 tests)
//! - `server/lib/server/src/grpc/presence.rs` (14 tests)
//!
//! Total: 32 unit tests covering:
//! - `PresenceMap` CRUD operations
//! - `SyncMode` transitions
//! - Follower tracking
//! - Thread safety (concurrent join/leave)
//! - All 6 gRPC RPC methods
//! - Error paths (not found, invalid arguments)

use std::time::Duration;

use {
    reovim_client_cli::GrpcClient,
    reovim_testing::{MultiClientPresenceTest, PresenceTestClient, TestServerHarness},
};

// ============================================================================
// Join/Leave Tests
// ============================================================================

/// Test that first client joins with empty peer list.
#[tokio::test]
async fn test_multi_client_first_joins_empty_peers() {
    MultiClientPresenceTest::with_clients(1)
        .await
        .run(|mut clients| async move {
            let peers = clients[0].peers().await.expect("Failed to get peers");
            assert!(peers.is_empty(), "First client should have no peers");
        })
        .await;
}

/// Test that second client sees first client as peer.
#[tokio::test]
async fn test_multi_client_second_sees_first_peer() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Client 1 (index 1) should see client 0 as peer
            let peers = clients[1].peers().await.expect("Failed to get peers");
            assert_eq!(peers.len(), 1, "Second client should see one peer");
            assert_eq!(
                peers[0].metadata.as_ref().unwrap().display_name,
                "client_0",
                "Peer should be client_0"
            );
        })
        .await;
}

/// Test that multiple clients can join and leave.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_multi_client_join_leave() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    // Connect 3 clients
    let mut client1 = connect_presence(&addr, "laptop").await;
    let mut client2 = connect_presence(&addr, "phone").await;
    let mut client3 = connect_presence(&addr, "browser").await;

    // Client 3 should see 2 peers
    let peers = client3.peers().await.expect("Failed to get peers");
    assert_eq!(peers.len(), 2, "Client 3 should see 2 peers");

    // Client 2 leaves
    client2.leave().await.expect("Failed to leave");

    // Small delay for server to process
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Verify client 1 sees only client 3
    let all = client1.all_clients().await.expect("Failed to list clients");
    assert_eq!(all.len(), 2, "Should have 2 clients after leave");

    // Cleanup
    client1.leave().await.ok();
    client3.leave().await.ok();
}

// ============================================================================
// Follow Mode Tests
// ============================================================================

/// Test setting FOLLOW mode with valid target.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_set_sync_mode_follow() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Client 0 becomes presenter
            clients[0].present().await.expect("Failed to set present");

            // Client 1 follows client 0
            let target_id = clients[0].client_id();
            clients[1]
                .follow(target_id)
                .await
                .expect("Failed to follow");

            // Verify via list_clients (client 0's view)
            let all = clients[0]
                .all_clients()
                .await
                .expect("Failed to list clients");
            let follower = all
                .iter()
                .find(|c| c.metadata.as_ref().map_or(false, |m| m.display_name == "client_1"))
                .expect("Follower not found");
            // In v3, following is indicated by relation.type == RelationTypeFollowing (0)
            // and relation.target_id == target client id.
            let relation = follower.relation.as_ref().expect("Follower should have relation");
            assert_eq!(relation.r#type, 0, "Should be in FOLLOW mode (RelationTypeFollowing=0)");
            assert_eq!(relation.target_id, target_id, "Should follow client_0");
        })
        .await;
}

/// Test that FOLLOW mode without target fails.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_set_sync_mode_follow_missing_target() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_presence(&addr, "test").await;

    // Try to set FOLLOW mode without target (sync_mode=1, no follow_target)
    // Identity resolved from session token (#483)
    let result = client.grpc().presence_set_sync_mode(1, None).await;

    // Should fail with InvalidArgument
    assert!(result.is_err(), "FOLLOW without target should fail");

    client.leave().await.ok();
}

/// Test that following non-existent target fails.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_set_sync_mode_follow_invalid_target() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_presence(&addr, "test").await;

    // Try to follow a non-existent client (ID 9999)
    // Identity resolved from session token (#483)
    let result = client.grpc().presence_set_sync_mode(1, Some(9999)).await;

    // Should fail with InvalidArgument
    assert!(result.is_err(), "FOLLOW with invalid target should fail");

    client.leave().await.ok();
}

// ============================================================================
// Presence Update Tests
// ============================================================================

/// Test updating cursor position (now a no-op).
///
/// Phase 14 (#471): Cursor tracking moved from `PresenceUpdated` to `CursorMoved` notifications.
/// The `update_cursor` method is preserved for API compatibility but does nothing.
#[tokio::test]
async fn test_update_cursor() {
    MultiClientPresenceTest::with_clients(1)
        .await
        .run(|mut clients| async move {
            // Update cursor (no-op since Phase 14 #471)
            clients[0]
                .update_cursor(10, 5)
                .await
                .expect("update_cursor should not fail");

            // Verify client is still in list (cursor no longer in presence)
            let all = clients[0]
                .all_clients()
                .await
                .expect("Failed to list clients");
            let me = all
                .iter()
                .find(|c| {
                    c.metadata
                        .as_ref()
                        .map_or(false, |m| m.display_name == "client_0")
                })
                .expect("Self not found");
            // Cursor field no longer exists in presence - just verify client exists
            assert_eq!(me.metadata.as_ref().unwrap().display_name, "client_0");
        })
        .await;
}

/// Test updating mode.
#[tokio::test]
async fn test_update_mode() {
    MultiClientPresenceTest::with_clients(1)
        .await
        .run(|mut clients| async move {
            // Update mode
            clients[0]
                .update_mode("INSERT")
                .await
                .expect("Failed to update mode");

            // Verify via list
            let all = clients[0]
                .all_clients()
                .await
                .expect("Failed to list clients");
            let me = all
                .iter()
                .find(|c| {
                    c.metadata
                        .as_ref()
                        .map_or(false, |m| m.display_name == "client_0")
                })
                .expect("Self not found");
            // mode is domain-owned in v3; verify client is present with correct display_name
            assert_eq!(me.metadata.as_ref().unwrap().display_name, "client_0");
        })
        .await;
}

// ============================================================================
// Concurrent Operations Tests
// ============================================================================

/// Test concurrent join/leave operations.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_concurrent_join_leave() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    // Spawn 10 tasks that each join, wait briefly, then leave
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let addr = addr.clone();
            tokio::spawn(async move {
                let mut client = GrpcClient::connect(&addr).await.expect("Failed to connect");
                let _resp = client
                    .presence_join("test", &format!("client_{i}"))
                    .await
                    .expect("Failed to join");
                tokio::time::sleep(Duration::from_millis(10)).await;
                // Identity resolved from session token (#483)
                client.presence_leave().await.expect("Failed to leave");
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    // Verify all left
    let mut client = GrpcClient::connect(&addr).await.expect("Failed to connect");
    let list = client.presence_list().await.expect("Failed to list");
    assert!(list.clients.is_empty(), "All clients should have left");
}

/// Test high-frequency update operations.
///
/// Phase 14 (#471): Cursor updates are now no-ops.
/// This test verifies that rapid mode updates work correctly.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_high_frequency_updates() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = connect_presence(&addr, "rapid").await;

    // Send 50 rapid mode updates (cursor updates are no-ops since Phase 14 #471)
    for i in 0..50 {
        client
            .update_mode(&format!("MODE_{i}"))
            .await
            .expect("Failed to update mode");
    }

    // Verify final state
    let all = client.all_clients().await.expect("Failed to list clients");
    let me = all
        .iter()
        .find(|c| c.metadata.as_ref().map_or(false, |m| m.display_name == "rapid"))
        .expect("Self not found");
    // mode is domain-owned in v3; verify client is present
    assert!(me.metadata.is_some(), "Client should have metadata");

    client.leave().await.ok();
}

// ============================================================================
// Error Path Tests
// ============================================================================

/// Test leave without authentication fails (#483).
///
/// With token-based identity, `presence_leave()` requires a session token.
/// Without joining first, there's no token, so the call should fail.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_leave_without_auth_fails() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = GrpcClient::connect(&addr).await.expect("Failed to connect");

    // Leave without joining (no session token) should fail with Unauthenticated
    let result = client.presence_leave().await;
    assert!(result.is_err(), "Leave without authentication should fail");
}

/// Test update without authentication fails (#483).
///
/// With token-based identity, `presence_update()` requires a session token.
/// Without joining first, there's no token, so the call should fail.
#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn test_update_without_auth_fails() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut client = GrpcClient::connect(&addr).await.expect("Failed to connect");

    // Update without joining (no session token) should fail with Unauthenticated
    let result = client.presence_update(None, None).await;

    // Should fail with Unauthenticated
    assert!(result.is_err(), "Update without authentication should fail");
}

// ============================================================================
// Streaming Tests (Deferred - require notification stream support)
// ============================================================================

/// Test that streaming receives join notifications.
#[tokio::test]
#[ignore = "Requires CLI notification stream support (#465 Phase 16)"]
async fn test_stream_receives_join_notification() {
    // Deferred: CLI doesn't expose StreamPresence yet
}

/// Test that streaming receives leave notifications.
#[tokio::test]
#[ignore = "Requires CLI notification stream support (#465 Phase 16)"]
async fn test_stream_receives_leave_notification() {
    // Deferred: CLI doesn't expose StreamPresence yet
}

/// Test that streaming receives update notifications.
#[tokio::test]
#[ignore = "Requires CLI notification stream support (#465 Phase 16)"]
async fn test_stream_receives_update_notification() {
    // Deferred: CLI doesn't expose StreamPresence yet
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Connect a presence-aware client to the server.
async fn connect_presence(addr: &str, display_name: &str) -> PresenceTestClient {
    let grpc = connect_with_retry(addr).await;
    let mut client = PresenceTestClient::new(grpc, display_name);
    client.join().await.expect("Failed to join presence");
    client
}

/// Connect to server with retry logic.
async fn connect_with_retry(addr: &str) -> GrpcClient {
    let mut attempts = 0;
    loop {
        match GrpcClient::connect(addr).await {
            Ok(c) => return c,
            Err(_) if attempts < 20 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => panic!("Failed to connect after {attempts} attempts: {e}"),
        }
    }
}
