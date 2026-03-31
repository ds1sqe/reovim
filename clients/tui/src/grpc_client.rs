//! gRPC v2 client for TUI.
//!
//! Provides a unified client interface for TUI communication with the server
//! using gRPC v2 protocol. Includes streaming notifications for real-time updates.
//!
//! # Services
//!
//! This client wraps all gRPC v2 services:
//! - `InputService` - Send keys to the editor
//! - `StateService` - Query mode, cursor, layout, options
//! - `BufferService` - Buffer content and file operations
//! - `ServerService` - Server management (ping, info, kill)
//! - `EditorService` - Editor operations (resize, quit, active buffer)
//! - `ModuleService` - Module listing
//! - `NotificationService` - Server-to-client streaming

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
    tonic::{Code, Request, Streaming, transport::Channel},
};

// ─────────────────────────────────────────────────────────────────────────────
// Error Handling (#479: Fail Loud with Client Panic)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle gRPC errors with panic vs retry policy.
///
/// # Phase #479: Client Panic Policy
///
/// - **PANIC** (misconfiguration/bug): `NotFound`, `InvalidArgument`, `PermissionDenied`,
///   `FailedPrecondition`, `Internal`, `Unimplemented`
/// - **PANIC** (transient - retry logic can be added later): `Unavailable`,
///   `ResourceExhausted`, `DeadlineExceeded`, `Aborted`
///
/// TUI should crash hard on server errors rather than continue with wrong state.
#[cfg_attr(coverage_nightly, coverage(off))]
fn handle_grpc_error(status: &tonic::Status, operation: &str, client_id: u64) -> ! {
    match status.code() {
        // PANIC - Client bug or misconfiguration
        Code::NotFound
        | Code::InvalidArgument
        | Code::PermissionDenied
        | Code::FailedPrecondition
        | Code::Internal
        | Code::Unimplemented => {
            panic!(
                "FATAL: {operation} failed for client_id={client_id}\n\
                 Code: {:?}\n\
                 Message: {}\n\
                 Ensure presence_join() succeeded.",
                status.code(),
                status.message()
            );
        }
        // RETRY - Transient issues (panic for now, retry logic can be added later)
        Code::Unavailable | Code::ResourceExhausted | Code::DeadlineExceeded | Code::Aborted => {
            panic!(
                "FATAL: {operation} failed for client_id={client_id} (transient)\n\
                 Code: {:?}\n\
                 Message: {}\n\
                 Server may be temporarily unavailable.",
                status.code(),
                status.message()
            );
        }
        // Unknown - log and panic
        _ => {
            panic!(
                "FATAL: {operation} failed for client_id={client_id} (unknown)\n\
                 Code: {:?}\n\
                 Message: {}",
                status.code(),
                status.message()
            );
        }
    }
}

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

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<tonic::transport::Error> for TuiGrpcError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::ConnectionFailed(err.to_string())
    }
}

/// gRPC v2 client for TUI.
///
/// Wraps all service clients and provides a unified interface for TUI operations.
/// This is the primary client for gRPC v2 communication.
///
/// `Clone` is cheap — tonic service clients share the underlying `Channel`.
#[derive(Clone)]
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
    /// Session token for token-based authentication (#483).
    ///
    /// Received from `Join()` response, sent as `x-reovim-token` metadata
    /// header on every subsequent gRPC request.
    session_token: Option<String>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

        // 64 MB message limit (matches server) for large buffer content
        const MAX: usize = 64 * 1024 * 1024;

        Ok(Self {
            input: InputServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            state: StateServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            buffer: BufferServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            notification: NotificationServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            server: ServerServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            editor: EditorServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            module: ModuleServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            syntax: SyntaxServiceClient::new(channel.clone()).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            presence: PresenceServiceClient::new(channel).max_decoding_message_size(MAX).max_encoding_message_size(MAX),
            session_token: None,
        })
    }

    /// Wrap a proto message in a `tonic::Request` with session token metadata (#483).
    ///
    /// If a session token has been stored (from `Join()`), injects it as the
    /// `x-reovim-token` gRPC metadata header. The server interceptor resolves
    /// this token to the caller's `ClientId`.
    fn make_request<T>(&self, body: T) -> Request<T> {
        let mut request = Request::new(body);
        if let Some(ref token) = self.session_token {
            request
                .metadata_mut()
                .insert("x-reovim-token", token.parse().expect("session token is valid ASCII"));
        }
        request
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Input Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Send keys to the editor.
    ///
    /// # Arguments
    ///
    /// * `keys` - Keys in vim notation (e.g., `iHello\<Esc\>`).
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn send_keys(&mut self, keys: &str) -> Result<SendKeysResponse, TuiGrpcError> {
        let request = self.make_request(SendKeysRequest {
            keys: keys.to_string(),
        });
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
        let request = self.make_request(GetModeRequest { client_id: 0 });
        let response = self.state.get_mode(request).await?;
        Ok(response.into_inner())
    }

    /// Get the mode for a specific client.
    ///
    /// # Per-client state (#471): Per-client mode isolation
    ///
    /// Returns the mode from the specified client's per-client mode stack.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_mode_for_client(
        &mut self,
        client_id: u64,
    ) -> Result<GetModeResponse, TuiGrpcError> {
        let request = self.make_request(GetModeRequest { client_id });
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
        let request = self.make_request(GetCursorRequest {
            window_id,
            client_id: 0,
        });
        let response = self.state.get_cursor(request).await?;
        Ok(response.into_inner())
    }

    /// Get the cursor position for a specific client.
    ///
    /// # Per-client state (#471): Per-client cursor isolation
    ///
    /// Returns the cursor from the specified client's per-client editing state.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_cursor_for_client(
        &mut self,
        window_id: Option<u64>,
        client_id: u64,
    ) -> Result<GetCursorResponse, TuiGrpcError> {
        let request = self.make_request(GetCursorRequest {
            window_id,
            client_id,
        });
        let response = self.state.get_cursor(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Panic Methods (#479: Fail Loud)
    // ─────────────────────────────────────────────────────────────────────────

    /// Get mode or panic - TUI should never continue with unknown mode.
    ///
    /// # Phase #479: Fail-Loud Policy
    ///
    /// TUI panics with traceback on server errors rather than continuing with
    /// potentially wrong state. After `presence_join()` succeeds, mode lookups
    /// should always work - failure indicates a bug or misconfiguration.
    ///
    /// # Panics
    ///
    /// Panics with detailed error message if the gRPC call fails.
    pub async fn get_mode_or_panic(&mut self, client_id: u64) -> GetModeResponse {
        match self.get_mode_for_client(client_id).await {
            Ok(resp) => resp,
            Err(TuiGrpcError::GrpcError(status)) => {
                handle_grpc_error(&status, "get_mode", client_id)
            }
            Err(e) => panic!(
                "FATAL: get_mode failed for client_id={client_id}\n\
                 Error: {e}\n\
                 Connection may have been lost."
            ),
        }
    }

    /// Get cursor or panic - TUI should never continue with unknown cursor.
    ///
    /// # Phase #479: Fail-Loud Policy
    ///
    /// TUI panics with traceback on server errors rather than continuing with
    /// potentially wrong state.
    ///
    /// # Panics
    ///
    /// Panics with detailed error message if the gRPC call fails.
    pub async fn get_cursor_or_panic(
        &mut self,
        window_id: Option<u64>,
        client_id: u64,
    ) -> GetCursorResponse {
        match self.get_cursor_for_client(window_id, client_id).await {
            Ok(resp) => resp,
            Err(TuiGrpcError::GrpcError(status)) => {
                handle_grpc_error(&status, "get_cursor", client_id)
            }
            Err(e) => panic!(
                "FATAL: get_cursor failed for client_id={client_id}\n\
                 Error: {e}\n\
                 Connection may have been lost."
            ),
        }
    }

    /// Get layout or panic - TUI should never continue with unknown layout.
    ///
    /// # Phase #479: Fail-Loud Policy
    ///
    /// TUI panics with traceback on server errors rather than continuing with
    /// potentially wrong state.
    ///
    /// # Panics
    ///
    /// Panics with detailed error message if the gRPC call fails.
    pub async fn get_layout_or_panic(&mut self, client_id: u64) -> GetLayoutResponse {
        match self.get_layout_for_client(client_id).await {
            Ok(resp) => resp,
            Err(TuiGrpcError::GrpcError(status)) => {
                handle_grpc_error(&status, "get_layout", client_id)
            }
            Err(e) => panic!(
                "FATAL: get_layout failed for client_id={client_id}\n\
                 Error: {e}\n\
                 Connection may have been lost."
            ),
        }
    }

    /// Get layout for a specific client.
    ///
    /// # Per-client state (#471): Per-client layout isolation
    ///
    /// Returns the layout from the specified client's per-client editing state.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_layout_for_client(
        &mut self,
        client_id: u64,
    ) -> Result<GetLayoutResponse, TuiGrpcError> {
        let request = self.make_request(GetLayoutRequest { client_id });
        let response = self.state.get_layout(request).await?;
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
        let request = self.make_request(ListBuffersRequest {});
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
        let request = self.make_request(GetRawContentRequest {
            buffer_id,
            start_line,
            end_line,
        });
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
        let request = self.make_request(SubscribeRequest { event_types });
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
        let request = self.make_request(PingRequest {});
        let response = self.server.ping(request).await?;
        Ok(response.into_inner())
    }

    /// Get server information.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn info(&mut self) -> Result<InfoResponse, TuiGrpcError> {
        let request = self.make_request(InfoRequest {});
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
        let request = self.make_request(KillRequest { force });
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
        let request = self.make_request(ResizeRequest { width, height });
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
        let request = self.make_request(QuitRequest { force });
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
        let request = self.make_request(SetActiveBufferRequest { buffer_id });
        let response = self.editor.set_active_buffer(request).await?;
        Ok(response.into_inner())
    }

    /// Get the active buffer ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_active_buffer(&mut self) -> Result<GetActiveBufferResponse, TuiGrpcError> {
        let request = self.make_request(GetActiveBufferRequest {});
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
        let request = self.make_request(GetLayoutRequest { client_id: 0 });
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
        let request = self.make_request(GetOptionsRequest { names });
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
        let request = self.make_request(GetSelectionRequest {
            window_id,
            client_id: 0,
        });
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
        let request = self.make_request(GetVisibleLinesRequest {
            window_id,
            client_id: 0,
        });
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
        let request = self.make_request(SubmitCaptureRequest {
            request_id,
            width,
            height,
            format: format.to_string(),
            content,
        });
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
        let request = self.make_request(OpenFileRequest {
            path: path.to_string(),
        });
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
        let request = self.make_request(WriteFileRequest { buffer_id, path });
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
        let request = self.make_request(ListModulesRequest {});
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
        let request = self.make_request(GetTokensRequest {
            buffer_id,
            start_line,
            end_line,
        });
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
        let request = self.make_request(StreamTokensRequest { buffer_id });
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
        let response = self.presence.join(request).await?.into_inner();
        // Store session token for subsequent requests (#483)
        if !response.session_token.is_empty() {
            self.session_token = Some(response.session_token.clone());
        }
        Ok(response)
    }

    /// Leave the presence session.
    ///
    /// Called on graceful disconnect to notify other clients.
    /// The server identifies the caller from the session token (#483).
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_leave(&mut self) -> Result<LeaveResponse, TuiGrpcError> {
        let request = self.make_request(LeaveRequest {});
        let response = self.presence.leave(request).await?;
        Ok(response.into_inner())
    }

    /// List all connected clients.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn presence_list(&mut self) -> Result<ListClientsResponse, TuiGrpcError> {
        let request = self.make_request(ListClientsRequest {});
        let response = self.presence.list_clients(request).await?;
        Ok(response.into_inner())
    }
}

#[cfg(test)]
#[path = "grpc_client_tests.rs"]
mod tests;
