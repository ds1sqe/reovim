//! gRPC v2 client for TUI.
//!
//! Provides a unified client interface for TUI communication with the server
//! using gRPC v2 protocol. Includes streaming notifications for real-time updates.

use {
    reovim_protocol::v2::{
        GetCursorRequest, GetCursorResponse, GetModeRequest, GetModeResponse, GetRawContentRequest,
        GetRawContentResponse, ListBuffersRequest, ListBuffersResponse, Notification,
        SendKeysRequest, SendKeysResponse, SubscribeRequest,
        buffer_service_client::BufferServiceClient, input_service_client::InputServiceClient,
        notification_service_client::NotificationServiceClient,
        state_service_client::StateServiceClient,
    },
    tonic::{Streaming, transport::Channel},
};

/// Error type for TUI gRPC client operations.
#[derive(Debug)]
pub enum TuiGrpcError {
    /// Failed to connect to the server.
    ConnectionFailed(String),
    /// gRPC call failed.
    GrpcError(tonic::Status),
}

impl std::fmt::Display for TuiGrpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::GrpcError(status) => write!(f, "gRPC error: {status}"),
        }
    }
}

impl std::error::Error for TuiGrpcError {}

impl From<tonic::Status> for TuiGrpcError {
    fn from(status: tonic::Status) -> Self {
        Self::GrpcError(status)
    }
}

impl From<tonic::transport::Error> for TuiGrpcError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::ConnectionFailed(err.to_string())
    }
}

/// gRPC v2 client for TUI.
///
/// Wraps all service clients (Input, State, Buffer, Notification) and provides
/// a unified interface for TUI operations.
pub struct TuiGrpcClient {
    input: InputServiceClient<Channel>,
    state: StateServiceClient<Channel>,
    buffer: BufferServiceClient<Channel>,
    notification: NotificationServiceClient<Channel>,
}

impl TuiGrpcClient {
    /// Connect to a gRPC server at the given address.
    ///
    /// # Arguments
    ///
    /// * `addr` - Server address in `host:port` format (e.g., "127.0.0.1:12540").
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(addr: &str) -> Result<Self, TuiGrpcError> {
        let url = format!("http://{addr}");
        let channel = Channel::from_shared(url)
            .map_err(|e| TuiGrpcError::ConnectionFailed(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            input: InputServiceClient::new(channel.clone()),
            state: StateServiceClient::new(channel.clone()),
            buffer: BufferServiceClient::new(channel.clone()),
            notification: NotificationServiceClient::new(channel),
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Input Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Send keys to the editor.
    ///
    /// # Arguments
    ///
    /// * `keys` - Keys in vim notation (e.g., "iHello<Esc>").
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn send_keys(&mut self, keys: &str) -> Result<SendKeysResponse, TuiGrpcError> {
        let request = SendKeysRequest {
            keys: keys.to_string(),
        };
        let response = self.input.send_keys(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // State Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the current editor mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_mode(&mut self) -> Result<GetModeResponse, TuiGrpcError> {
        let request = GetModeRequest {};
        let response = self.state.get_mode(request).await?;
        Ok(response.into_inner())
    }

    /// Get the cursor position.
    ///
    /// # Arguments
    ///
    /// * `window_id` - Optional window ID. Uses focused window if None.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_cursor(
        &mut self,
        window_id: Option<u64>,
    ) -> Result<GetCursorResponse, TuiGrpcError> {
        let request = GetCursorRequest { window_id };
        let response = self.state.get_cursor(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Buffer Service
    // ─────────────────────────────────────────────────────────────────────────

    /// List all open buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn list_buffers(&mut self) -> Result<ListBuffersResponse, TuiGrpcError> {
        let request = ListBuffersRequest {};
        let response = self.buffer.list(request).await?;
        Ok(response.into_inner())
    }

    /// Get raw buffer content.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - Optional buffer ID. Uses active buffer if None.
    /// * `start_line` - Optional start line (0-indexed).
    /// * `end_line` - Optional end line (exclusive).
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_buffer_content(
        &mut self,
        buffer_id: Option<u64>,
        start_line: Option<u64>,
        end_line: Option<u64>,
    ) -> Result<GetRawContentResponse, TuiGrpcError> {
        let request = GetRawContentRequest {
            buffer_id,
            start_line,
            end_line,
        };
        let response = self.buffer.get_raw_content(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Notification Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Subscribe to notifications (server streaming).
    ///
    /// Returns a stream of notifications for state changes.
    ///
    /// # Arguments
    ///
    /// * `event_types` - Event types to subscribe to (empty = all events).
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    pub async fn subscribe(
        &mut self,
        event_types: Vec<String>,
    ) -> Result<Streaming<Notification>, TuiGrpcError> {
        let request = SubscribeRequest { event_types };
        let response = self.notification.subscribe(request).await?;
        Ok(response.into_inner())
    }

    /// Subscribe to all notifications.
    ///
    /// Convenience method that subscribes to all event types.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    pub async fn subscribe_all(&mut self) -> Result<Streaming<Notification>, TuiGrpcError> {
        self.subscribe(vec![]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TuiGrpcError::ConnectionFailed("test".to_string());
        assert!(err.to_string().contains("Connection failed"));

        let status = tonic::Status::not_found("test");
        let err = TuiGrpcError::GrpcError(status);
        assert!(err.to_string().contains("gRPC error"));
    }

    #[test]
    fn test_error_from_status() {
        let status = tonic::Status::internal("internal error");
        let err: TuiGrpcError = status.into();
        matches!(err, TuiGrpcError::GrpcError(_));
    }
}
