//! JSON-RPC client for communicating with a reovim server.
//!
//! Provides request/response handling and notification receiving.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use {
    reovim_protocol::v1::{RpcNotification, RpcRequest, RpcResponse},
    serde_json::Value,
};

use super::connection::{Connection, ConnectionConfig, ConnectionReader, ConnectionWriter};

/// Default RPC call timeout (30 seconds).
const RPC_TIMEOUT_SECS: u64 = 30;

/// Error type for RPC client operations.
#[derive(Debug)]
pub enum RpcClientError {
    /// Connection error.
    Connection(std::io::Error),
    /// JSON serialization/deserialization error.
    Json(serde_json::Error),
    /// Server returned an error response.
    Server { code: i32, message: String },
    /// Unexpected response (wrong ID or format).
    UnexpectedResponse(String),
    /// RPC call timed out.
    Timeout,
}

impl std::fmt::Display for RpcClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(e) => write!(f, "Connection error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Server { code, message } => write!(f, "Server error ({code}): {message}"),
            Self::UnexpectedResponse(msg) => write!(f, "Unexpected response: {msg}"),
            Self::Timeout => write!(f, "RPC call timed out after {RPC_TIMEOUT_SECS} seconds"),
        }
    }
}

impl std::error::Error for RpcClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connection(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for RpcClientError {
    fn from(e: std::io::Error) -> Self {
        Self::Connection(e)
    }
}

impl From<serde_json::Error> for RpcClientError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Message received from server.
#[derive(Debug)]
pub enum ServerMessage {
    /// Response to a request.
    Response(RpcResponse),
    /// Server-initiated notification.
    Notification(RpcNotification),
}

/// Writer half of a split RPC client.
///
/// Created by [`RpcClient::into_split`]. Used for sending requests
/// while a separate task handles incoming messages via [`ConnectionReader`].
pub struct RpcWriter {
    writer: ConnectionWriter,
    next_id: AtomicU64,
}

impl RpcWriter {
    /// Send a request without waiting for response.
    ///
    /// Returns the request ID that can be used to correlate responses.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or write fails.
    pub async fn send_request(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<u64, RpcClientError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = RpcRequest::new(id, method, params);
        let json = serde_json::to_string(&request)?;
        self.writer.write_line(&json).await?;
        Ok(id)
    }

    /// Send a notification (no response expected).
    ///
    /// # Errors
    ///
    /// Returns error if serialization or write fails.
    pub async fn send_notification(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<(), RpcClientError> {
        let notification = RpcNotification::new(method, params);
        let json = serde_json::to_string(&notification)?;
        self.writer.write_line(&json).await?;
        Ok(())
    }
}

/// JSON-RPC client for reovim server.
pub struct RpcClient {
    connection: Connection,
    next_id: AtomicU64,
}

impl RpcClient {
    /// Connect to a server.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, RpcClientError> {
        let connection = Connection::connect(config).await?;
        Ok(Self {
            connection,
            next_id: AtomicU64::new(1),
        })
    }

    /// Send a request and wait for response.
    ///
    /// Times out after 30 seconds by default.
    ///
    /// # Errors
    ///
    /// Returns error if request fails, server returns an error, or timeout expires.
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, RpcClientError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        // Build request
        let request = RpcRequest::new(id, method, params);
        let json = serde_json::to_string(&request)?;

        // Send request
        self.connection.write_line(&json).await?;

        // Wait for response with timeout protection
        tokio::time::timeout(Duration::from_secs(RPC_TIMEOUT_SECS), self.wait_for_response(id))
            .await
            .map_err(|_| RpcClientError::Timeout)?
    }

    /// Wait for response with matching ID.
    ///
    /// Internal helper that loops until a response arrives or an error occurs.
    async fn wait_for_response(&mut self, id: u64) -> Result<Value, RpcClientError> {
        loop {
            let line = self.connection.read_line().await?;

            // Try to parse as response first
            if let Ok(response) = serde_json::from_str::<RpcResponse>(&line) {
                if response.id == id {
                    // Check for error
                    if let Some(error) = response.error {
                        return Err(RpcClientError::Server {
                            code: error.code,
                            message: error.message,
                        });
                    }
                    return Ok(response.result.unwrap_or(Value::Null));
                }
                // Wrong ID, unexpected
                return Err(RpcClientError::UnexpectedResponse(format!(
                    "Expected response ID {id}, got {}",
                    response.id
                )));
            }

            // Try to parse as notification (ignore for now, will be handled by read_message)
            if serde_json::from_str::<RpcNotification>(&line).is_ok() {
                // Skip notifications while waiting for response
                continue;
            }

            // Unknown message format
            return Err(RpcClientError::UnexpectedResponse(format!(
                "Unknown message format: {line}"
            )));
        }
    }

    /// Read next message from server (response or notification).
    ///
    /// Use this in event loops to handle both responses and notifications.
    ///
    /// # Errors
    ///
    /// Returns error if read fails or message format is invalid.
    pub async fn read_message(&mut self) -> Result<ServerMessage, RpcClientError> {
        let line = self.connection.read_line().await?;

        // Try to parse as response
        if let Ok(response) = serde_json::from_str::<RpcResponse>(&line) {
            return Ok(ServerMessage::Response(response));
        }

        // Try to parse as notification
        if let Ok(notification) = serde_json::from_str::<RpcNotification>(&line) {
            return Ok(ServerMessage::Notification(notification));
        }

        Err(RpcClientError::UnexpectedResponse(format!("Unknown message format: {line}")))
    }

    /// Send a raw JSON string.
    ///
    /// # Errors
    ///
    /// Returns error if send or receive fails.
    pub async fn send_raw(&mut self, json: &str) -> Result<Value, RpcClientError> {
        self.connection.write_line(json).await?;

        let line = self.connection.read_line().await?;
        let response: RpcResponse = serde_json::from_str(&line)?;

        if let Some(error) = response.error {
            return Err(RpcClientError::Server {
                code: error.code,
                message: error.message,
            });
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Split client into reader and writer for concurrent operation.
    ///
    /// Use this when you need to:
    /// - Listen for notifications in a background task
    /// - Send requests from the main task
    /// - Handle both concurrently (e.g., TUI client)
    ///
    /// Returns the connection reader (for spawning a notification listener)
    /// and an RPC writer (for sending requests).
    #[must_use]
    pub fn into_split(self) -> (ConnectionReader, RpcWriter) {
        let (reader, writer) = self.connection.split();
        (
            reader,
            RpcWriter {
                writer,
                next_id: self.next_id,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = RpcClientError::Server {
            code: -32600,
            message: "Invalid Request".to_string(),
        };
        assert!(err.to_string().contains("-32600"));
        assert!(err.to_string().contains("Invalid Request"));
    }

    #[test]
    fn test_timeout_error_message() {
        let err = RpcClientError::Timeout;
        let msg = err.to_string();
        assert!(msg.contains("timed out"), "Should mention timeout: {msg}");
        assert!(msg.contains("30"), "Should mention 30 seconds: {msg}");
    }

    #[test]
    fn test_timeout_constant() {
        // Verify the timeout is 30 seconds as specified in issue #226
        assert_eq!(RPC_TIMEOUT_SECS, 30);
    }

    #[test]
    fn test_error_variants() {
        // Test all error variants can be created and displayed
        let errors: Vec<RpcClientError> = vec![
            RpcClientError::Connection(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "test",
            )),
            RpcClientError::Json(serde_json::from_str::<()>("invalid").unwrap_err()),
            RpcClientError::Server {
                code: -32000,
                message: "Test".to_string(),
            },
            RpcClientError::UnexpectedResponse("test".to_string()),
            RpcClientError::Timeout,
        ];

        for err in errors {
            // Should not panic when converting to string
            let _ = err.to_string();
        }
    }
}
