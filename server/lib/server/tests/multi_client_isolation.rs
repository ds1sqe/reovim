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

/// Test that undo correctly adjusts positions when edits are position-dependent (#495).
///
/// This verifies the OT-lite transformation: when Client 0 makes an edit and
/// Client 1 then inserts text BEFORE Client 0's edit position, Client 0's undo
/// must transform inverse edit positions to account for Client 1's text shift,
/// removing only Client 0's text.
///
/// Without OT-lite, the inverse Delete would use the original position (0,0),
/// deleting Client 1's "BBBB" instead of Client 0's "AAAA".
#[tokio::test]
async fn test_undo_with_dependent_edits() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Client 0 types "AAAA" at position (0,0)
            clients[0]
                .send_keys("iAAAA<Esc>")
                .await
                .expect("client 0 types AAAA");
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Client 1 inserts "BBBB" at position 0 (before Client 0's text)
            // Use 0i to explicitly go to column 0 before entering insert mode
            clients[1]
                .send_keys("0iBBBB<Esc>")
                .await
                .expect("client 1 types BBBB");
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Buffer should contain both BBBB and AAAA
            let content_before = clients[0]
                .get_buffer()
                .await
                .expect("get buffer before undo");
            assert!(
                content_before.contains("BBBB") && content_before.contains("AAAA"),
                "Buffer should contain both BBBB and AAAA, got: '{content_before}'"
            );

            // Client 0 undoes — OT-lite transforms the inverse position
            // from (0,0) to (0,4), correctly targeting "AAAA" not "BBBB"
            clients[0].send_keys("u").await.expect("client 0 undo");
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Buffer should contain only "BBBB" (Client 0's AAAA was removed)
            let content_after = clients[0]
                .get_buffer()
                .await
                .expect("get buffer after undo");
            assert!(
                content_after.contains("BBBB") && !content_after.contains("AAAA"),
                "After OT-lite undo: should contain 'BBBB' but not 'AAAA', got: '{content_after}'"
            );
        })
        .await;
}

/// Test that undo is isolated per-client (#471).
///
/// When Client 0 makes edits and Client 1 makes edits, then Client 0
/// undoes, only Client 0's edits should be undone.
///
/// This test verifies the multi-client undo architecture where each
/// client's 'u' command only undoes their own changes.
#[tokio::test]
async fn test_undo_isolation() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Setup: Both clients start with empty buffer

            // Client 0 types "AAA"
            clients[0]
                .send_keys("iAAA<Esc>")
                .await
                .expect("client 0 types AAA");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Client 1 types "BBB" (appends after AAA)
            clients[1]
                .send_keys("$aBBB<Esc>")
                .await
                .expect("client 1 types BBB");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Buffer should now contain "AAABBB"
            let content_before = clients[0]
                .get_buffer()
                .await
                .expect("get buffer before undo");
            assert!(
                content_before.contains("AAABBB"),
                "Buffer should contain AAABBB before undo, got: {content_before}"
            );

            // Client 0 undoes - should only undo their "AAA", not Client 1's "BBB"
            clients[0].send_keys("u").await.expect("client 0 undo");
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Buffer should now contain only "BBB" (Client 0's AAA was undone)
            let content_after = clients[0]
                .get_buffer()
                .await
                .expect("get buffer after undo");
            assert!(
                content_after.contains("BBB") && !content_after.contains("AAA"),
                "After Client 0 undo: buffer should contain 'BBB' but not 'AAA', got: {content_after}"
            );
        })
        .await;
}
