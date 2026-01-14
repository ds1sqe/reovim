//! Client struct representing a connected client.
//!
//! Each client has a unique ID and is attached to a session.
//! The client owns a response channel for sending messages back.

use std::sync::Arc;

use tokio::{
    io::{AsyncWriteExt, BufWriter},
    net::tcp::OwnedWriteHalf,
    sync::Mutex,
};

use crate::session::{ClientId, SessionId};

/// A connected client.
///
/// Represents a single client connection to the server. Each client:
/// - Has a unique [`ClientId`]
/// - Is attached to a [`Session`] via [`SessionId`]
/// - Owns a writer for sending responses
///
/// # Thread Safety
///
/// The writer is wrapped in `tokio::sync::Mutex` to allow concurrent
/// access from different tasks (e.g., event broadcasts). This matches
/// the lock hierarchy in `docs/reference/concurrency.md`:
/// Level 2 (Per-Client) - `Mutex<WriteHalf>`.
///
/// # Example
///
/// ```ignore
/// use runner::client::Client;
/// use runner::session::{ClientId, SessionId};
///
/// // Create client from TCP connection
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

    /// Buffered writer for sending responses.
    ///
    /// Protected by mutex to allow concurrent sends from different tasks.
    writer: Mutex<BufWriter<OwnedWriteHalf>>,
}

impl Client {
    /// Create a new client.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique client ID (from `SessionRegistry::next_client_id()`)
    /// * `session_id` - The session this client is attached to
    /// * `writer` - The TCP write half for this connection
    #[must_use]
    pub fn new(id: ClientId, session_id: SessionId, writer: OwnedWriteHalf) -> Arc<Self> {
        Arc::new(Self {
            id,
            session_id,
            writer: Mutex::new(BufWriter::new(writer)),
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
        let mut writer = self.writer.lock().await;
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await
    }

    /// Send raw bytes to the client.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub async fn send_bytes(&self, data: &[u8]) -> std::io::Result<()> {
        let mut writer = self.writer.lock().await;
        writer.write_all(data).await?;
        writer.flush().await
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
