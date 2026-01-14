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
//! // Broadcast mode change to all clients
//! let notification = RpcNotification {
//!     method: "mode/changed".to_string(),
//!     params: serde_json::json!({"mode": "Insert"}),
//! };
//! NotificationBroadcaster::broadcast_to_session(&session, notification).await;
//!
//! // Broadcast to all except the triggering client
//! NotificationBroadcaster::broadcast_except(&session, notification, client_id).await;
//! ```

use crate::session::{ClientId, Session};

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
        crate::session::SessionId,
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
    };

    fn test_session() -> std::sync::Arc<Session> {
        Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            ModeId::new(ModuleId::new("test"), "normal"),
        )
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

    #[test]
    fn test_client_count_empty() {
        let session = test_session();
        assert_eq!(NotificationBroadcaster::client_count(&session), 0);
    }
}
