//! Multi-client state isolation tests (#471).
//!
//! Verifies that cursor, selection, and mode are isolated per-client.
//! These tests use the `MultiClientPresenceTest` infrastructure which
//! automatically joins clients to the presence service.

use std::time::Duration;

use reovim_testing::MultiClientPresenceTest;

/// Test that mode changes are isolated per-client.
///
/// When Client 0 enters INSERT mode, Client 1 should remain in NORMAL mode.
#[tokio::test]
async fn test_mode_isolation() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Both clients start in NORMAL mode
            let mode0 = clients[0].get_mode().await.expect("get mode 0");
            let mode1 = clients[1].get_mode().await.expect("get mode 1");
            assert!(
                mode0.display.to_uppercase().contains("NORMAL"),
                "Client 0 should start in NORMAL, got: {}",
                mode0.display
            );
            assert!(
                mode1.display.to_uppercase().contains("NORMAL"),
                "Client 1 should start in NORMAL, got: {}",
                mode1.display
            );

            // Client 0 enters insert mode
            clients[0]
                .send_keys("i")
                .await
                .expect("send keys to client 0");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Client 0 should be in INSERT mode
            let mode0 = clients[0]
                .get_mode()
                .await
                .expect("get mode 0 after insert");
            assert!(
                mode0.display.to_uppercase().contains("INSERT"),
                "Client 0 should be in INSERT, got: {}",
                mode0.display
            );

            // Client 1 should STILL be in NORMAL mode (isolated)
            let mode1 = clients[1]
                .get_mode()
                .await
                .expect("get mode 1 after client 0 insert");
            assert!(
                mode1.display.to_uppercase().contains("NORMAL"),
                "Client 1 mode should be NORMAL after Client 0 enters INSERT, got: {}",
                mode1.display
            );

            // Cleanup: Client 0 exits insert mode
            clients[0].send_keys("<Esc>").await.ok();
        })
        .await;
}

/// Test that cursor movements are isolated per-client.
///
/// When Client 0 moves their cursor, Client 1's cursor should not change.
#[tokio::test]
async fn test_cursor_isolation() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Per-client cursor tracking is now fully implemented (#471).
            // Each client has independent cursor position in their active window.

            // Get initial cursor positions
            let cursor0 = clients[0].get_cursor().await.expect("get cursor 0");
            let cursor1 = clients[1].get_cursor().await.expect("get cursor 1");

            // Both should start at (0, 0)
            assert_eq!(cursor0, (0, 0), "Client 0 should start at (0, 0), got: {cursor0:?}");
            assert_eq!(cursor1, (0, 0), "Client 1 should start at (0, 0), got: {cursor1:?}");

            // Client 0 moves down
            clients[0].send_keys("j").await.expect("send j to client 0");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Client 1 cursor should NOT have moved
            let cursor1_after = clients[1]
                .get_cursor()
                .await
                .expect("get cursor 1 after client 0 move");
            assert_eq!(
                cursor1_after,
                (0, 0),
                "Client 1 cursor should not move when Client 0 moves, got: {cursor1_after:?}"
            );
        })
        .await;
}

/// Test that visual selection is isolated per-client.
///
/// When Client 0 enters visual mode and selects text, Client 1 should
/// remain in normal mode without any selection.
#[tokio::test]
async fn test_selection_isolation() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Client 0 enters visual mode and selects
            clients[0]
                .send_keys("vl")
                .await
                .expect("send vl to client 0");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Client 0 should be in VISUAL mode
            let mode0 = clients[0]
                .get_mode()
                .await
                .expect("get mode 0 after visual");
            assert!(
                mode0.display.to_uppercase().contains("VISUAL"),
                "Client 0 should be in VISUAL, got: {}",
                mode0.display
            );

            // Client 1 should still be in NORMAL mode (no selection)
            let mode1 = clients[1]
                .get_mode()
                .await
                .expect("get mode 1 after client 0 visual");
            assert!(
                mode1.display.to_uppercase().contains("NORMAL"),
                "Client 1 should not enter visual mode when Client 0 does, got: {}",
                mode1.display
            );

            // Cleanup: Client 0 exits visual mode
            clients[0].send_keys("<Esc>").await.ok();
        })
        .await;
}

/// Test that both clients can independently type in different modes.
///
/// Client 0 types in insert mode while Client 1 remains in normal mode,
/// verifying complete mode isolation.
#[tokio::test]
async fn test_independent_editing() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Client 0 enters insert mode and types
            clients[0]
                .send_keys("iHello")
                .await
                .expect("send iHello to client 0");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Verify Client 0 is in INSERT mode
            let mode0 = clients[0].get_mode().await.expect("get mode 0");
            assert!(
                mode0.display.to_uppercase().contains("INSERT"),
                "Client 0 should be in INSERT, got: {}",
                mode0.display
            );

            // Verify Client 1 is still in NORMAL mode
            let mode1 = clients[1].get_mode().await.expect("get mode 1");
            assert!(
                mode1.display.to_uppercase().contains("NORMAL"),
                "Client 1 should remain in NORMAL, got: {}",
                mode1.display
            );

            // Client 1 should be able to send normal mode commands
            // without affecting Client 0's insert mode
            clients[1].send_keys("j").await.expect("send j to client 1");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Client 0 should still be in INSERT mode
            let mode0_after = clients[0]
                .get_mode()
                .await
                .expect("get mode 0 after client 1 j");
            assert!(
                mode0_after.display.to_uppercase().contains("INSERT"),
                "Client 0 should still be in INSERT after Client 1's j command, got: {}",
                mode0_after.display
            );

            // Cleanup
            clients[0].send_keys("<Esc>").await.ok();
        })
        .await;
}
