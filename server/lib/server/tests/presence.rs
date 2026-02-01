//! Integration tests for `PresenceService` (Phase 14, Epic #465).
//!
//! These tests verify multi-client presence awareness works end-to-end.
//!
//! # Test Categories
//!
//! | Category | Description |
//! |----------|-------------|
//! | Join/Leave | Client lifecycle operations |
//! | Streaming | Presence update streaming |
//! | Follow Mode | Sync mode functionality |
//! | Concurrent | Thread safety under load |
//!
//! # Prerequisites
//!
//! Integration tests require:
//! 1. Server binary built with modules
//! 2. CLI client with presence RPC support (not yet implemented)
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

use reovim_testing::TestServerHarness;

/// Helper to spawn server with retry.
async fn spawn_server() -> TestServerHarness {
    TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server")
}

// ============================================================================
// Join/Leave Tests
// ============================================================================

/// Test that first client joins with empty peer list.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_multi_client_first_joins_empty_peers() {
    let _harness = spawn_server().await;
    // TODO: When CLI client supports presence:
    // let mut client = connect(&harness).await;
    // let response = client.presence_join("tui", "laptop").await;
    // assert!(response.peers.is_empty());
}

/// Test that second client sees first client as peer.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_multi_client_second_sees_first_peer() {
    let _harness = spawn_server().await;
    // TODO: When CLI client supports presence:
    // let mut client1 = connect(&harness).await;
    // client1.presence_join("tui", "laptop").await;
    //
    // let mut client2 = connect(&harness).await;
    // let response = client2.presence_join("android", "phone").await;
    // assert_eq!(response.peers.len(), 1);
    // assert_eq!(response.peers[0].display_name, "laptop");
}

/// Test that multiple clients can join and leave.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_multi_client_join_leave() {
    let _harness = spawn_server().await;
    // TODO: When CLI client supports presence:
    // let mut client1 = connect(&harness).await;
    // let resp1 = client1.presence_join("tui", "laptop").await;
    //
    // let mut client2 = connect(&harness).await;
    // client2.presence_join("android", "phone").await;
    //
    // let mut client3 = connect(&harness).await;
    // let resp3 = client3.presence_join("web", "browser").await;
    // assert_eq!(resp3.peers.len(), 2);
    //
    // // Client 2 leaves
    // client2.presence_leave(resp2.client_id).await;
    //
    // // Verify via list_clients
    // let clients = client1.presence_list().await;
    // assert_eq!(clients.len(), 2);
}

// ============================================================================
// Streaming Tests
// ============================================================================

/// Test that streaming receives join notifications.
#[tokio::test]
#[ignore = "Requires CLI client with presence streaming support (#465)"]
async fn test_stream_receives_join_notification() {
    let _harness = spawn_server().await;
    // TODO: When CLI client supports presence streaming:
    // let mut client1 = connect(&harness).await;
    // client1.presence_join("tui", "observer").await;
    // let mut stream = client1.presence_stream().await;
    //
    // // Client 2 joins
    // let mut client2 = connect(&harness).await;
    // client2.presence_join("android", "phone").await;
    //
    // // Observer should receive joined notification
    // let update = tokio::time::timeout(
    //     Duration::from_secs(1),
    //     stream.next()
    // ).await.expect("Timeout").expect("Stream ended");
    //
    // assert!(matches!(update.update, Some(Update::Joined(_))));
}

/// Test that streaming receives leave notifications.
#[tokio::test]
#[ignore = "Requires CLI client with presence streaming support (#465)"]
async fn test_stream_receives_leave_notification() {
    let _harness = spawn_server().await;
    // Similar to above, but test Leave notification
}

/// Test that streaming receives update notifications.
#[tokio::test]
#[ignore = "Requires CLI client with presence streaming support (#465)"]
async fn test_stream_receives_update_notification() {
    let _harness = spawn_server().await;
    // Test that cursor/viewport updates emit notifications
}

// ============================================================================
// Follow Mode Tests
// ============================================================================

/// Test setting FOLLOW mode with valid target.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_set_sync_mode_follow() {
    let _harness = spawn_server().await;
    // TODO:
    // let mut presenter = connect(&harness).await;
    // let resp1 = presenter.presence_join("tui", "presenter").await;
    // presenter.presence_set_sync_mode(resp1.client_id, SyncMode::Present, None).await;
    //
    // let mut follower = connect(&harness).await;
    // let resp2 = follower.presence_join("android", "follower").await;
    // follower.presence_set_sync_mode(resp2.client_id, SyncMode::Follow, Some(resp1.client_id)).await;
    //
    // // Verify via list_clients
    // let clients = presenter.presence_list().await;
    // let follower_info = clients.iter().find(|c| c.display_name == "follower").unwrap();
    // assert_eq!(follower_info.sync_mode, SyncMode::Follow as i32);
}

/// Test that FOLLOW mode without target fails.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_set_sync_mode_follow_missing_target() {
    let _harness = spawn_server().await;
    // Should return InvalidArgument
}

/// Test that following non-existent target fails.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_set_sync_mode_follow_invalid_target() {
    let _harness = spawn_server().await;
    // Should return InvalidArgument
}

// ============================================================================
// Concurrent Operations Tests
// ============================================================================

/// Test concurrent join/leave operations.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_concurrent_join_leave() {
    let _harness = spawn_server().await;
    // Spawn 10 tasks that each:
    // 1. Join
    // 2. Update presence a few times
    // 3. Leave
    // Verify no panics or data races
}

/// Test high-frequency update operations.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_high_frequency_updates() {
    let _harness = spawn_server().await;
    // Single client sending rapid cursor updates
    // Verify all updates are processed correctly
}

// ============================================================================
// Error Path Tests
// ============================================================================

/// Test leave for unknown client returns ok: false.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_leave_unknown_client() {
    let _harness = spawn_server().await;
    // client.presence_leave(9999) should return { ok: false }
}

/// Test update for unknown client returns `NotFound`.
#[tokio::test]
#[ignore = "Requires CLI client with presence RPC support (#465)"]
async fn test_update_unknown_client() {
    let _harness = spawn_server().await;
    // client.presence_update(9999, ...) should return NotFound error
}
