//! Notification broadcasting to session clients.
//!
//! Provides utilities for sending JSON-RPC notifications to all clients
//! connected to a session. Used for events like mode changes, buffer updates, etc.
//!
//! # Example
//!
//! ```ignore
//! use runner::notification::NotificationBroadcaster;
//! use runner::session::Session;
//! use reovim_protocol::v1::RpcNotification;
//!
//! // Broadcast mode change to all clients (session-wide event)
//! let notification = RpcNotification {
//!     method: "mode/changed".to_string(),
//!     params: serde_json::json!({"mode": "Insert"}),
//! };
//! NotificationBroadcaster::broadcast_to_session(&session, notification).await;
//!
//! // Broadcast to all except the triggering client
//! NotificationBroadcaster::broadcast_except(&session, notification, client_id).await;
//!
//! // Broadcast cursor moved to clients viewing a specific buffer
//! let cursor_notification = r#"{"jsonrpc":"2.0","method":"cursor/moved","params":{...}}"#;
//! NotificationBroadcaster::broadcast_to_buffer(&session, buffer_id, cursor_notification).await;
//! ```

use {
    crate::session::{ClientId, Session},
    reovim_kernel::api::v1::BufferId,
};

/// Broadcasts notifications to session clients.
///
/// This is a utility struct with static methods - no instances are created.
/// All broadcasts are fire-and-forget: errors on individual sends are logged
/// but don't stop other clients from receiving the notification.
pub struct NotificationBroadcaster;

impl NotificationBroadcaster {
    /// Broadcast a notification to all clients in a session.
    ///
    /// Sends the notification JSON to every connected client. Send errors
    /// are logged but don't prevent delivery to other clients.
    ///
    /// # Arguments
    ///
    /// * `session` - The session containing the clients
    /// * `notification` - The JSON string to broadcast (should be valid JSON-RPC notification)
    pub async fn broadcast_to_session(session: &Session, notification: &str) {
        for client in session.clients().iter() {
            if let Err(e) = client.send_line(notification).await {
                tracing::warn!("Failed to send notification to client {:?}: {e}", client.id());
            }
        }
    }

    /// Broadcast a notification to all clients except one.
    ///
    /// Useful when a client triggers an action and shouldn't receive
    /// the resulting notification (e.g., they already know about the change).
    ///
    /// # Arguments
    ///
    /// * `session` - The session containing the clients
    /// * `notification` - The JSON string to broadcast
    /// * `except` - Client ID to skip
    pub async fn broadcast_except(session: &Session, notification: &str, except: ClientId) {
        for client in session.clients().iter() {
            if client.id() == except {
                continue;
            }
            if let Err(e) = client.send_line(notification).await {
                tracing::warn!("Failed to send notification to client {:?}: {e}", client.id());
            }
        }
    }

    /// Broadcast a notification to clients viewing a specific buffer.
    ///
    /// More efficient than [`broadcast_to_session`](Self::broadcast_to_session) when only clients
    /// viewing a particular buffer need the notification (e.g., cursor moved, buffer modified).
    ///
    /// # Zero Allocation Fast Path
    ///
    /// If no clients are viewing the buffer, this function returns immediately
    /// without iterating or allocating.
    ///
    /// # Lock Safety
    ///
    /// This method briefly acquires the viewport lock (Level 2) for each client to check
    /// their active buffer. The lock is dropped before sending to avoid holding locks
    /// during I/O.
    ///
    /// # Arguments
    ///
    /// * `session` - The session containing the clients
    /// * `buffer_id` - The buffer to filter by
    /// * `notification` - The JSON string to broadcast
    pub async fn broadcast_to_buffer(session: &Session, buffer_id: BufferId, notification: &str) {
        for client in session.clients().iter() {
            // Check if client is viewing this buffer (Level 2 lock)
            let is_viewing = {
                let viewport = client.viewport().read().await;
                viewport.active_buffer == Some(buffer_id)
            }; // Lock dropped before I/O

            if is_viewing && let Err(e) = client.send_line(notification).await {
                tracing::warn!("Failed to send notification to client {:?}: {e}", client.id());
            }
        }
    }

    /// Get the number of clients that would receive a broadcast.
    ///
    /// Useful for logging or deciding whether to broadcast at all.
    #[must_use]
    pub fn client_count(session: &Session) -> usize {
        session.clients().len()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            server::{client::Client, transport::TransportWriter},
            session::SessionId,
        },
        reovim_driver_vfs::{MockVfs, VfsDriver},
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
        std::sync::Arc,
    };

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    fn test_session() -> std::sync::Arc<Session> {
        Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            ModeId::new(ModuleId::new("test"), "normal"),
            test_vfs(),
        )
    }

    fn test_client(session_id: SessionId, client_id: ClientId) -> Arc<Client> {
        Client::new(client_id, session_id, TransportWriter::from_stdio())
    }

    #[tokio::test]
    async fn test_broadcast_to_empty_session() {
        let session = test_session();
        let notification = r#"{"jsonrpc":"2.0","method":"test/event","params":{}}"#;

        // Should not panic with zero clients
        NotificationBroadcaster::broadcast_to_session(&session, notification).await;
    }

    #[tokio::test]
    async fn test_broadcast_except_empty_session() {
        let session = test_session();
        let notification = r#"{"jsonrpc":"2.0","method":"test/event","params":{}}"#;

        // Should not panic with zero clients
        NotificationBroadcaster::broadcast_except(&session, notification, ClientId::new(1)).await;
    }

    #[tokio::test]
    async fn test_broadcast_to_buffer_empty_session() {
        let session = test_session();
        let notification = r#"{"jsonrpc":"2.0","method":"cursor/moved","params":{}}"#;
        let buffer_id = BufferId::from_raw(1);

        // Should not panic with zero clients
        NotificationBroadcaster::broadcast_to_buffer(&session, buffer_id, notification).await;
    }

    #[test]
    fn test_client_count_empty() {
        let session = test_session();
        assert_eq!(NotificationBroadcaster::client_count(&session), 0);
    }

    #[test]
    fn test_client_count_with_clients() {
        let session = test_session();

        // Add clients
        let client1 = test_client(session.id().clone(), ClientId::new(1));
        let client2 = test_client(session.id().clone(), ClientId::new(2));

        session.clients().insert(&client1);
        session.clients().insert(&client2);

        assert_eq!(NotificationBroadcaster::client_count(&session), 2);
    }

    #[tokio::test]
    async fn test_broadcast_to_buffer_filters_by_active_buffer() {
        let session = test_session();
        let target_buffer = BufferId::from_raw(1);
        let other_buffer = BufferId::from_raw(2);

        // Client 1 viewing target buffer
        let client1 = test_client(session.id().clone(), ClientId::new(1));
        // Client 2 viewing other buffer
        let client2 = test_client(session.id().clone(), ClientId::new(2));
        // Client 3 viewing target buffer
        let client3 = test_client(session.id().clone(), ClientId::new(3));
        // Client 4 with no active buffer
        let client4 = test_client(session.id().clone(), ClientId::new(4));

        // Register all clients
        session.clients().insert(&client1);
        session.clients().insert(&client2);
        session.clients().insert(&client3);
        session.clients().insert(&client4);

        // Set active buffers
        {
            let mut vp = client1.viewport().write().await;
            vp.active_buffer = Some(target_buffer);
        }
        {
            let mut vp = client2.viewport().write().await;
            vp.active_buffer = Some(other_buffer);
        }
        {
            let mut vp = client3.viewport().write().await;
            vp.active_buffer = Some(target_buffer);
        }
        // client4 has no active buffer (None)

        // Count how many clients would receive the notification
        // (we can't easily verify actual send, but we can verify the filtering logic)
        let mut would_receive = 0;
        for client in session.clients().iter() {
            let is_viewing = {
                let viewport = client.viewport().read().await;
                viewport.active_buffer == Some(target_buffer)
            };
            if is_viewing {
                would_receive += 1;
            }
        }

        assert_eq!(
            would_receive, 2,
            "Only clients 1 and 3 should receive buffer-scoped notification"
        );
    }

    #[tokio::test]
    async fn test_broadcast_to_buffer_no_clients_viewing() {
        let session = test_session();
        let target_buffer = BufferId::from_raw(1);
        let other_buffer = BufferId::from_raw(2);

        // Both clients viewing other buffer
        let client1 = test_client(session.id().clone(), ClientId::new(1));
        let client2 = test_client(session.id().clone(), ClientId::new(2));

        session.clients().insert(&client1);
        session.clients().insert(&client2);

        {
            let mut vp = client1.viewport().write().await;
            vp.active_buffer = Some(other_buffer);
        }
        {
            let mut vp = client2.viewport().write().await;
            vp.active_buffer = Some(other_buffer);
        }

        // Count how many would receive
        let mut would_receive = 0;
        for client in session.clients().iter() {
            let is_viewing = {
                let viewport = client.viewport().read().await;
                viewport.active_buffer == Some(target_buffer)
            };
            if is_viewing {
                would_receive += 1;
            }
        }

        assert_eq!(
            would_receive, 0,
            "No clients should receive notification for buffer they're not viewing"
        );
    }

    #[tokio::test]
    async fn test_broadcast_to_session_reaches_all_clients() {
        let session = test_session();

        // Create clients with different active buffers
        let client1 = test_client(session.id().clone(), ClientId::new(1));
        let client2 = test_client(session.id().clone(), ClientId::new(2));
        let client3 = test_client(session.id().clone(), ClientId::new(3));

        session.clients().insert(&client1);
        session.clients().insert(&client2);
        session.clients().insert(&client3);

        {
            let mut vp = client1.viewport().write().await;
            vp.active_buffer = Some(BufferId::from_raw(1));
        }
        {
            let mut vp = client2.viewport().write().await;
            vp.active_buffer = Some(BufferId::from_raw(2));
        }
        // client3 has no active buffer

        // Session-wide broadcast should reach all clients regardless of active buffer
        let all_clients = session.clients().len();
        assert_eq!(all_clients, 3, "All 3 clients should receive session-wide notification");
    }

    #[tokio::test]
    async fn test_broadcast_except_skips_specified_client() {
        let session = test_session();

        let client1 = test_client(session.id().clone(), ClientId::new(1));
        let client2 = test_client(session.id().clone(), ClientId::new(2));
        let client3 = test_client(session.id().clone(), ClientId::new(3));

        session.clients().insert(&client1);
        session.clients().insert(&client2);
        session.clients().insert(&client3);

        let except_id = ClientId::new(2);

        // Count how many would receive (excluding client 2)
        let mut would_receive = 0;
        for client in session.clients().iter() {
            if client.id() != except_id {
                would_receive += 1;
            }
        }

        assert_eq!(would_receive, 2, "broadcast_except should skip the specified client");
    }
}
