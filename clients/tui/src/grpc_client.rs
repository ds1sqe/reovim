//! gRPC v2 client for TUI.
//!
//! Provides a unified client interface for TUI communication with the server
//! using gRPC v2 protocol. Includes streaming notifications for real-time updates.
//!
//! # Services
//!
//! This client wraps all gRPC v2 services:
//! - [`InputService`] - Send keys to the editor
//! - [`StateService`] - Query mode, cursor, layout, options
//! - [`BufferService`] - Buffer content and file operations
//! - [`ServerService`] - Server management (ping, info, kill)
//! - [`EditorService`] - Editor operations (resize, quit, active buffer)
//! - [`ModuleService`] - Module listing
//! - [`NotificationService`] - Server-to-client streaming

use {
    reovim_protocol::v2::{
        GetActiveBufferRequest, GetActiveBufferResponse, GetCursorRequest, GetCursorResponse,
        GetLayoutRequest, GetLayoutResponse, GetModeRequest, GetModeResponse, GetOptionsRequest,
        GetOptionsResponse, GetRawContentRequest, GetRawContentResponse, GetSelectionRequest,
        GetSelectionResponse, GetTokensRequest, GetTokensResponse, GetVisibleLinesRequest,
        GetVisibleLinesResponse, InfoRequest, InfoResponse, JoinRequest, JoinResponse, KillRequest,
        KillResponse, LeaveRequest, LeaveResponse, ListBuffersRequest, ListBuffersResponse,
        ListClientsRequest, ListClientsResponse, ListModulesRequest, ListModulesResponse,
        Notification, OpenFileRequest, OpenFileResponse, PingRequest, PingResponse, QuitRequest,
        QuitResponse, ResizeRequest, ResizeResponse, SendKeysRequest, SendKeysResponse,
        SetActiveBufferRequest, SetActiveBufferResponse, StreamTokensRequest, SubmitCaptureRequest,
        SubmitCaptureResponseReply, SubscribeRequest, TokenUpdate, WriteFileRequest,
        WriteFileResponse, buffer_service_client::BufferServiceClient,
        editor_service_client::EditorServiceClient, input_service_client::InputServiceClient,
        module_service_client::ModuleServiceClient,
        notification_service_client::NotificationServiceClient,
        presence_service_client::PresenceServiceClient, server_service_client::ServerServiceClient,
        state_service_client::StateServiceClient, syntax_service_client::SyntaxServiceClient,
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
/// Wraps all service clients and provides a unified interface for TUI operations.
/// This is the primary client for gRPC v2 communication.
pub struct TuiGrpcClient {
    input: InputServiceClient<Channel>,
    state: StateServiceClient<Channel>,
    buffer: BufferServiceClient<Channel>,
    notification: NotificationServiceClient<Channel>,
    server: ServerServiceClient<Channel>,
    editor: EditorServiceClient<Channel>,
    module: ModuleServiceClient<Channel>,
    syntax: SyntaxServiceClient<Channel>,
    presence: PresenceServiceClient<Channel>,
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
            notification: NotificationServiceClient::new(channel.clone()),
            server: ServerServiceClient::new(channel.clone()),
            editor: EditorServiceClient::new(channel.clone()),
            module: ModuleServiceClient::new(channel.clone()),
            syntax: SyntaxServiceClient::new(channel.clone()),
            presence: PresenceServiceClient::new(channel),
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
            client_id: None, // Use default client (Phase 11.2)
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

    // ─────────────────────────────────────────────────────────────────────────
    // Server Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Ping the server.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn ping(&mut self) -> Result<PingResponse, TuiGrpcError> {
        let request = PingRequest {};
        let response = self.server.ping(request).await?;
        Ok(response.into_inner())
    }

    /// Get server information.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn info(&mut self) -> Result<InfoResponse, TuiGrpcError> {
        let request = InfoRequest {};
        let response = self.server.info(request).await?;
        Ok(response.into_inner())
    }

    /// Kill the server.
    ///
    /// # Arguments
    ///
    /// * `force` - If true, force kill without graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn kill(&mut self, force: bool) -> Result<KillResponse, TuiGrpcError> {
        let request = KillRequest { force };
        let response = self.server.kill(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Editor Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Resize the editor viewport.
    ///
    /// # Arguments
    ///
    /// * `width` - New viewport width.
    /// * `height` - New viewport height.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn resize(
        &mut self,
        width: u64,
        height: u64,
    ) -> Result<ResizeResponse, TuiGrpcError> {
        let request = ResizeRequest { width, height };
        let response = self.editor.resize(request).await?;
        Ok(response.into_inner())
    }

    /// Quit the editor.
    ///
    /// # Arguments
    ///
    /// * `force` - If true, discard unsaved changes.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn quit(&mut self, force: bool) -> Result<QuitResponse, TuiGrpcError> {
        let request = QuitRequest { force };
        let response = self.editor.quit(request).await?;
        Ok(response.into_inner())
    }

    /// Set the active buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - Buffer ID to make active.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn set_active_buffer(
        &mut self,
        buffer_id: u64,
    ) -> Result<SetActiveBufferResponse, TuiGrpcError> {
        let request = SetActiveBufferRequest { buffer_id };
        let response = self.editor.set_active_buffer(request).await?;
        Ok(response.into_inner())
    }

    /// Get the active buffer ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_active_buffer(&mut self) -> Result<GetActiveBufferResponse, TuiGrpcError> {
        let request = GetActiveBufferRequest {};
        let response = self.editor.get_active_buffer(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Extended State Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Get window layout.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_layout(&mut self) -> Result<GetLayoutResponse, TuiGrpcError> {
        let request = GetLayoutRequest {};
        let response = self.state.get_layout(request).await?;
        Ok(response.into_inner())
    }

    /// Get editor options.
    ///
    /// # Arguments
    ///
    /// * `names` - Option names to query (empty = all options).
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_options(
        &mut self,
        names: Vec<String>,
    ) -> Result<GetOptionsResponse, TuiGrpcError> {
        let request = GetOptionsRequest { names };
        let response = self.state.get_options(request).await?;
        Ok(response.into_inner())
    }

    /// Get selection state.
    ///
    /// # Arguments
    ///
    /// * `window_id` - Optional window ID. Uses focused window if None.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_selection(
        &mut self,
        window_id: Option<u64>,
    ) -> Result<GetSelectionResponse, TuiGrpcError> {
        let request = GetSelectionRequest { window_id };
        let response = self.state.get_selection(request).await?;
        Ok(response.into_inner())
    }

    /// Get visible lines for a window.
    ///
    /// # Arguments
    ///
    /// * `window_id` - Optional window ID. Uses focused window if None.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_visible_lines(
        &mut self,
        window_id: Option<u64>,
    ) -> Result<GetVisibleLinesResponse, TuiGrpcError> {
        let request = GetVisibleLinesRequest { window_id };
        let response = self.state.get_visible_lines(request).await?;
        Ok(response.into_inner())
    }

    /// Submit captured screen content (TUI → Server part of capture relay).
    ///
    /// Called in response to a `capture_request` notification.
    ///
    /// # Arguments
    ///
    /// * `request_id` - Must match the `request_id` from the `capture_request` notification
    /// * `width` - Frame width
    /// * `height` - Frame height
    /// * `format` - Format used for capture (`plain_text`, `raw_ansi`, `cell_grid`)
    /// * `content` - Captured frame content
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn submit_capture_response(
        &mut self,
        request_id: u64,
        width: u64,
        height: u64,
        format: &str,
        content: String,
    ) -> Result<SubmitCaptureResponseReply, TuiGrpcError> {
        let request = SubmitCaptureRequest {
            request_id,
            width,
            height,
            format: format.to_string(),
            content,
        };
        let response = self.state.submit_capture_response(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Extended Buffer Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Open a file.
    ///
    /// # Arguments
    ///
    /// * `path` - File path to open.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn open_file(&mut self, path: &str) -> Result<OpenFileResponse, TuiGrpcError> {
        let request = OpenFileRequest {
            path: path.to_string(),
        };
        let response = self.buffer.open_file(request).await?;
        Ok(response.into_inner())
    }

    /// Write buffer to file.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - Optional buffer ID. Uses active buffer if None.
    /// * `path` - Optional path. Uses buffer's path if None.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn write_file(
        &mut self,
        buffer_id: Option<u64>,
        path: Option<String>,
    ) -> Result<WriteFileResponse, TuiGrpcError> {
        let request = WriteFileRequest { buffer_id, path };
        let response = self.buffer.write_file(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Module Service
    // ─────────────────────────────────────────────────────────────────────────

    /// List loaded modules.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn list_modules(&mut self) -> Result<ListModulesResponse, TuiGrpcError> {
        let request = ListModulesRequest {};
        let response = self.module.list(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Syntax Service (Phase 13.0 #470)
    // ─────────────────────────────────────────────────────────────────────────

    /// Get syntax tokens for a buffer range.
    ///
    /// One-shot query for initial buffer load or after major edits.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - Buffer to get tokens for.
    /// * `start_line` - Optional start line (default: 0).
    /// * `end_line` - Optional end line (default: end of buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_tokens(
        &mut self,
        buffer_id: u64,
        start_line: Option<u64>,
        end_line: Option<u64>,
    ) -> Result<GetTokensResponse, TuiGrpcError> {
        let request = GetTokensRequest {
            buffer_id,
            start_line,
            end_line,
        };
        let response = self.syntax.get_tokens(request).await?;
        Ok(response.into_inner())
    }

    /// Subscribe to real-time syntax token updates.
    ///
    /// Returns a stream of `TokenUpdate` messages for incremental highlighting.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - Buffer to stream tokens for.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    pub async fn stream_tokens(
        &mut self,
        buffer_id: u64,
    ) -> Result<Streaming<TokenUpdate>, TuiGrpcError> {
        let request = StreamTokensRequest { buffer_id };
        let response = self.syntax.stream_tokens(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Presence Service (Phase 11.2 #465)
    // ─────────────────────────────────────────────────────────────────────────

    /// Join the presence session to get a unique client ID.
    ///
    /// Each client MUST call this on connect to get an independent identity.
    /// Without a unique `client_id`, all clients share state (cursor, mode, etc.).
    ///
    /// # Arguments
    ///
    /// * `client_type` - Client type identifier ("tui", "web", "cli").
    /// * `display_name` - User-friendly display name for this client.
    ///
    /// # Returns
    ///
    /// Response containing the assigned `client_id` and list of connected peers.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_join(
        &mut self,
        client_type: &str,
        display_name: &str,
    ) -> Result<JoinResponse, TuiGrpcError> {
        let request = JoinRequest {
            client_type: client_type.to_string(),
            display_name: display_name.to_string(),
        };
        let response = self.presence.join(request).await?;
        Ok(response.into_inner())
    }

    /// Leave the presence session.
    ///
    /// Called on graceful disconnect to notify other clients.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client ID to remove.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_leave(&mut self, client_id: u64) -> Result<LeaveResponse, TuiGrpcError> {
        let request = LeaveRequest { client_id };
        let response = self.presence.leave(request).await?;
        Ok(response.into_inner())
    }

    /// List all connected clients.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_list(&mut self) -> Result<ListClientsResponse, TuiGrpcError> {
        let request = ListClientsRequest {};
        let response = self.presence.list_clients(request).await?;
        Ok(response.into_inner())
    }

    /// Send keys to the editor with explicit client ID.
    ///
    /// This is the preferred method for sending keys. It ensures each client's
    /// input is routed to their own state (cursor, mode, etc.) rather than
    /// sharing state with all other clients.
    ///
    /// # Arguments
    ///
    /// * `keys` - Keys in vim notation (e.g., "iHello<Esc>").
    /// * `client_id` - The client ID from `presence_join()`.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn send_keys_with_client(
        &mut self,
        keys: &str,
        client_id: u64,
    ) -> Result<SendKeysResponse, TuiGrpcError> {
        let request = SendKeysRequest {
            keys: keys.to_string(),
            client_id: Some(client_id),
        };
        let response = self.input.send_keys(request).await?;
        Ok(response.into_inner())
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
