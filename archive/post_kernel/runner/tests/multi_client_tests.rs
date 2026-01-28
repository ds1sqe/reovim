//! Multi-client concurrent tests.
//!
//! **Status**: 3 tests enabled, all passing.
//!
//! Tests verify that multiple clients can connect to the same server
//! and see changes made by other clients.

use runner::testing::MultiClientTest;

#[tokio::test]
async fn test_two_clients_read_same_buffer() {
    MultiClientTest::with_clients(2)
        .await
        .with_buffer("")
        .run(|clients| async move {
            // Client 0 opens a buffer first
            clients[0].open_buffer("").await.unwrap();

            // Client 0 inserts
            clients[0].send_keys("ihello<Esc>").await.unwrap();

            // Small delay for sync
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Client 1 reads
            let content = clients[1].get_buffer().await.unwrap();
            assert!(content.contains("hello"));
        })
        .await;
}

#[tokio::test]
async fn test_clients_see_changes() {
    MultiClientTest::with_clients(2)
        .await
        .with_buffer("")
        .run(|clients| async move {
            // Client 0 opens a buffer first
            clients[0].open_buffer("").await.unwrap();

            // Client 0 makes change
            clients[0].send_keys("iworld<Esc>").await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Client 1 should see it
            let content = clients[1].get_buffer().await.unwrap();
            assert!(content.contains("world"));
        })
        .await;
}

#[tokio::test]
async fn test_three_clients_concurrent() {
    MultiClientTest::with_clients(3)
        .await
        .with_buffer("")
        .run(|clients| async move {
            // All clients should connect successfully
            for (i, client) in clients.iter().enumerate() {
                let cursor = client.get_cursor().await;
                assert!(cursor.is_ok(), "Client {i} should get cursor");
            }
        })
        .await;
}
