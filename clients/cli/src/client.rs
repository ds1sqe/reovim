//! gRPC v2 client wrapper.
//!
//! Provides a unified client interface to all gRPC v2 services.

use {
    reovim_protocol::v2::{
        GetCursorRequest,
        GetCursorResponse,
        GetModeRequest,
        GetModeResponse,
        GetRawContentRequest,
        GetRawContentResponse,
        GetRegistersRequest,
        GetRegistersResponse,
        GetScreenContentRequest,
        GetScreenContentResponse,
        InfoRequest,
        InfoResponse,
        // Phase 15: Presence types
        JoinRequest,
        JoinResponse,
        LeaveRequest,
        LeaveResponse,
        ListBuffersRequest,
        ListBuffersResponse,
        ListClientsRequest,
        ListClientsResponse,
        PingRequest,
        PingResponse,
        SendKeysRequest,
        SendKeysResponse,
        SetSyncModeRequest,
        SetSyncModeResponse,
        UpdatePresenceRequest,
        UpdatePresenceResponse,
        buffer_service_client::BufferServiceClient,
        input_service_client::InputServiceClient,
        presence_service_client::PresenceServiceClient,
        server_service_client::ServerServiceClient,
        state_service_client::StateServiceClient,
    },
    tonic::transport::Channel,
};

/// Error type for gRPC client operations.
#[derive(Debug)]
pub enum GrpcClientError {
    /// Failed to connect to the server.
    ConnectionFailed(String),
    /// gRPC call failed.
    GrpcError(tonic::Status),
}

impl std::fmt::Display for GrpcClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::GrpcError(status) => write!(f, "gRPC error: {status}"),
        }
    }
}

impl std::error::Error for GrpcClientError {}

impl From<tonic::Status> for GrpcClientError {
    fn from(status: tonic::Status) -> Self {
        Self::GrpcError(status)
    }
}

impl From<tonic::transport::Error> for GrpcClientError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::ConnectionFailed(err.to_string())
    }
}

/// gRPC v2 client for interacting with the reovim server.
///
/// Wraps all service clients (Input, State, Buffer, Server, Presence) and provides
/// a unified interface.
pub struct GrpcClient {
    input: InputServiceClient<Channel>,
    state: StateServiceClient<Channel>,
    buffer: BufferServiceClient<Channel>,
    server: ServerServiceClient<Channel>,
    presence: PresenceServiceClient<Channel>,
}

impl GrpcClient {
    /// Connect to a gRPC server at the given address.
    ///
    /// # Arguments
    ///
    /// * `addr` - Server address in `host:port` format (e.g., "127.0.0.1:12540").
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(addr: &str) -> Result<Self, GrpcClientError> {
        // Build the endpoint URL
        let url = format!("http://{addr}");
        let channel = Channel::from_shared(url)
            .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            input: InputServiceClient::new(channel.clone()),
            state: StateServiceClient::new(channel.clone()),
            buffer: BufferServiceClient::new(channel.clone()),
            server: ServerServiceClient::new(channel.clone()),
            presence: PresenceServiceClient::new(channel),
        })
    }

    /// Send keys to the editor.
    ///
    /// # Arguments
    ///
    /// * `keys` - Keys in vim notation (e.g., "iHello<Esc>").
    ///
    /// # Returns
    ///
    /// The response containing success status and key status.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn send_keys(&mut self, keys: &str) -> Result<SendKeysResponse, GrpcClientError> {
        let request = SendKeysRequest {
            keys: keys.to_string(),
        };
        let response = self.input.send_keys(request).await?;
        Ok(response.into_inner())
    }

    /// Get the current editor mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_mode(&mut self) -> Result<GetModeResponse, GrpcClientError> {
        let request = GetModeRequest {};
        let response = self.state.get_mode(request).await?;
        Ok(response.into_inner())
    }

    /// Get the cursor position.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_cursor(&mut self) -> Result<GetCursorResponse, GrpcClientError> {
        let request = GetCursorRequest { window_id: None };
        let response = self.state.get_cursor(request).await?;
        Ok(response.into_inner())
    }

    /// List all open buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn list_buffers(&mut self) -> Result<ListBuffersResponse, GrpcClientError> {
        let request = ListBuffersRequest {};
        let response = self.buffer.list(request).await?;
        Ok(response.into_inner())
    }

    /// Get raw buffer content.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - Optional buffer ID. Uses active buffer if None.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_buffer_content(
        &mut self,
        buffer_id: Option<u64>,
    ) -> Result<GetRawContentResponse, GrpcClientError> {
        let request = GetRawContentRequest {
            buffer_id,
            start_line: None,
            end_line: None,
        };
        let response = self.buffer.get_raw_content(request).await?;
        Ok(response.into_inner())
    }

    /// Ping the server (health check).
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn ping(&mut self) -> Result<PingResponse, GrpcClientError> {
        let request = PingRequest {};
        let response = self.server.ping(request).await?;
        Ok(response.into_inner())
    }

    /// Get server info.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn info(&mut self) -> Result<InfoResponse, GrpcClientError> {
        let request = InfoRequest {};
        let response = self.server.info(request).await?;
        Ok(response.into_inner())
    }

    /// Get register contents.
    ///
    /// # Arguments
    ///
    /// * `names` - Optional register names to query. If empty, returns all non-empty registers.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_registers(
        &mut self,
        names: Vec<String>,
    ) -> Result<GetRegistersResponse, GrpcClientError> {
        let request = GetRegistersRequest { names };
        let response = self.state.get_registers(request).await?;
        Ok(response.into_inner())
    }

    /// Get screen content via TUI capture relay.
    ///
    /// Requests a screen capture from the connected TUI client via the server.
    /// Requires a headless TUI to be connected.
    ///
    /// # Arguments
    ///
    /// * `format` - Capture format: `plain_text`, `raw_ansi`, or `cell_grid`
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails, no TUI is connected, or capture times out.
    pub async fn get_screen_content(
        &mut self,
        format: &str,
    ) -> Result<GetScreenContentResponse, GrpcClientError> {
        let request = GetScreenContentRequest {
            format: format.to_string(),
        };
        let response = self.state.get_screen_content(request).await?;
        Ok(response.into_inner())
    }

    // =========================================================================
    // Presence Service Methods (Phase 15)
    // =========================================================================

    /// Join the presence session.
    ///
    /// Registers this client with the session and receives an assigned client ID.
    ///
    /// # Arguments
    ///
    /// * `client_type` - Client type identifier ("cli", "tui", "web", "test").
    /// * `display_name` - User-friendly display name.
    ///
    /// # Returns
    ///
    /// The response containing assigned client ID and list of connected peers.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_join(
        &mut self,
        client_type: &str,
        display_name: &str,
    ) -> Result<JoinResponse, GrpcClientError> {
        let request = JoinRequest {
            client_type: client_type.to_string(),
            display_name: display_name.to_string(),
        };
        let response = self.presence.join(request).await?;
        Ok(response.into_inner())
    }

    /// Leave the presence session.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client ID to remove.
    ///
    /// # Returns
    ///
    /// The response indicating success or failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_leave(
        &mut self,
        client_id: u64,
    ) -> Result<LeaveResponse, GrpcClientError> {
        let request = LeaveRequest { client_id };
        let response = self.presence.leave(request).await?;
        Ok(response.into_inner())
    }

    /// List all connected clients.
    ///
    /// # Returns
    ///
    /// The response containing all connected clients.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_list(&mut self) -> Result<ListClientsResponse, GrpcClientError> {
        let request = ListClientsRequest {};
        let response = self.presence.list_clients(request).await?;
        Ok(response.into_inner())
    }

    /// Update this client's presence state.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client ID making the update.
    /// * `buffer_id` - Optional new buffer ID.
    /// * `cursor_line` - Optional cursor line.
    /// * `cursor_column` - Optional cursor column.
    /// * `mode` - Optional mode name.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_update(
        &mut self,
        client_id: u64,
        buffer_id: Option<u64>,
        cursor_line: Option<u64>,
        cursor_column: Option<u64>,
        mode: Option<String>,
    ) -> Result<UpdatePresenceResponse, GrpcClientError> {
        use reovim_protocol::v2::Position;

        let cursor = match (cursor_line, cursor_column) {
            (Some(line), Some(column)) => Some(Position { line, column }),
            _ => None,
        };

        let request = UpdatePresenceRequest {
            client_id,
            buffer_id,
            cursor,
            visible_lines: None,
            mode,
        };
        let response = self.presence.update_presence(request).await?;
        Ok(response.into_inner())
    }

    /// Set sync mode for this client.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client ID setting the mode.
    /// * `sync_mode` - The sync mode (0 = Independent, 1 = Follow, 2 = Present).
    /// * `follow_target` - Target client ID when mode is Follow.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_set_sync_mode(
        &mut self,
        client_id: u64,
        sync_mode: i32,
        follow_target: Option<u64>,
    ) -> Result<SetSyncModeResponse, GrpcClientError> {
        let request = SetSyncModeRequest {
            client_id,
            mode: sync_mode,
            follow_target,
        };
        let response = self.presence.set_sync_mode(request).await?;
        Ok(response.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = GrpcClientError::ConnectionFailed("test".to_string());
        assert!(err.to_string().contains("Connection failed"));

        let status = tonic::Status::not_found("test");
        let err = GrpcClientError::GrpcError(status);
        assert!(err.to_string().contains("gRPC error"));
    }
}
