//! Client struct representing a connected client.
//!
//! Each client has a unique ID and is attached to a session.
//! The client owns a response channel for sending messages back.

use std::sync::Arc;

use crate::{
    session::{ClientId, SessionId},
    transport::TransportWriter,
};

/// A connected client.
///
/// Represents a single client connection to the server. Each client:
/// - Has a unique [`ClientId`]
/// - Is attached to a session via [`SessionId`]
/// - Owns a writer for sending responses
///
/// # Thread Safety
///
/// The [`TransportWriter`] uses an internal `tokio::sync::Mutex` for thread-safe
/// concurrent writes. This matches the lock hierarchy in `docs/reference/concurrency.md`:
/// Level 2 (Per-Client) - `Mutex<WriteHalf>`.
///
/// # Transport Support
///
/// The client works with any transport type (TCP, Unix socket, Stdio)
/// through the generic `TransportWriter` abstraction.
///
/// # Example
///
/// ```ignore
/// use runner::client::Client;
/// use runner::session::{ClientId, SessionId};
/// use runner::transport::TransportWriter;
///
/// // Create client from any transport
/// let client = Client::new(ClientId::new(1), SessionId::default(), writer);
///
/// // Send a response
/// client.send_line(r#"{"jsonrpc":"2.0","result":"ok","id":1}"#).await?;
/// ```
pub struct Client {
    /// Unique client identifier.
    id: ClientId,

    /// Session this client is attached to.
    session_id: SessionId,

    /// Writer for sending responses.
    ///
    /// Supports TCP, Unix socket, and Stdio transports.
    /// Has internal locking for thread-safe concurrent sends.
    writer: TransportWriter,
}

impl Client {
    /// Create a new client.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique client ID (from `SessionRegistry::next_client_id()`)
    /// * `session_id` - The session this client is attached to
    /// * `writer` - The transport writer for this connection
    #[must_use]
    pub fn new(id: ClientId, session_id: SessionId, writer: TransportWriter) -> Arc<Self> {
        Arc::new(Self {
            id,
            session_id,
            writer,
        })
    }

    /// Get the client ID.
    #[must_use]
    pub const fn id(&self) -> ClientId {
        self.id
    }

    /// Get the session ID this client is attached to.
    #[must_use]
    pub const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Send a line to the client.
    ///
    /// Appends a newline and flushes the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub async fn send_line(&self, line: &str) -> std::io::Result<()> {
        self.writer.write_line(line).await
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("id", &self.id)
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require actual TCP connections.
    // Unit tests here focus on struct creation and accessors.

    #[test]
    fn test_client_id_accessor() {
        // We can't easily test Client::new without a real socket,
        // so we verify the ClientId and SessionId work correctly
        let client_id = ClientId::new(42);
        let session_id = SessionId::new("test");

        assert_eq!(client_id.value(), 42);
        assert_eq!(session_id.name(), "test");
    }
}
