//! JSON-RPC client for communicating with a reovim server.
//!
//! Provides request/response handling and notification receiving.

use std::sync::atomic::{AtomicU64, Ordering};

use {
    reovim_protocol::v1::{RpcNotification, RpcRequest, RpcResponse},
    serde_json::Value,
};

use super::connection::{Connection, ConnectionConfig};

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
}

impl std::fmt::Display for RpcClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(e) => write!(f, "Connection error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Server { code, message } => write!(f, "Server error ({code}): {message}"),
            Self::UnexpectedResponse(msg) => write!(f, "Unexpected response: {msg}"),
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
    /// # Errors
    ///
    /// Returns error if request fails or server returns an error.
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, RpcClientError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        // Build request
        let request = RpcRequest::new(id, method, params);
        let json = serde_json::to_string(&request)?;

        // Send request
        self.connection.write_line(&json).await?;

        // Wait for response with matching ID
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
}
