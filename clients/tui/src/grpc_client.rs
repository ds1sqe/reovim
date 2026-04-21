//! gRPC v3 client for TUI.
//!
//! Provides a unified client interface for TUI communication with the server
//! using gRPC v3 protocol. Includes streaming notifications for real-time updates.
//!
//! # Services
//!
//! This client wraps all gRPC v3 services:
//! - `InputService` - Send keys to the editor
//! - `StateService` - Query projections, layout, options
//! - `BufferService` - Buffer metadata and file operations
//! - `ServerService` - Server management (ping, info, kill)
//! - `EditorService` - Editor operations (surface_changed, quit, active buffer)
//! - `ModuleService` - Module listing
//! - `NotificationService` - Server-to-client streaming
//! - `PresenceService` - Multi-client session management

use {
    reovim_protocol::v3::{
        GetActiveBufferRequest, GetActiveBufferResponse, GetLayoutRequest, GetLayoutResponse,
        GetOptionsRequest, GetOptionsResponse, GetProjectionsRequest, InfoRequest, InfoResponse,
        JoinRequest, JoinResponse, KillRequest, KillResponse, LeaveRequest, LeaveResponse,
        ListBuffersRequest, ListBuffersResponse, ListClientsRequest, ListClientsResponse,
        ListModulesRequest, ListModulesResponse, Notification, OpenFileRequest, OpenFileResponse,
        PingRequest, PingResponse, ProjectionEntry, QuitRequest, QuitResponse, SendInputRequest,
        SendInputResponse, SetActiveBufferRequest, SetActiveBufferResponse, SubmitCaptureRequest,
        SubmitCaptureResponseReply, SubscribeRequest, SurfaceChangedRequest,
        SurfaceDescriptorProto, WriteFileRequest, WriteFileResponse,
        buffer_service_client::BufferServiceClient, editor_service_client::EditorServiceClient,
        input_service_client::InputServiceClient, module_service_client::ModuleServiceClient,
        notification_service_client::NotificationServiceClient,
        presence_service_client::PresenceServiceClient, server_service_client::ServerServiceClient,
        state_service_client::StateServiceClient,
    },
    tonic::{Code, Request, Streaming, transport::Channel},
};

/// Maximum gRPC message size (encoding and decoding) in bytes.
///
/// Matches the server-side limit. Required for large buffer content
/// such as hex dumps of binary files.
const GRPC_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

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

/// Stub result for `get_buffer_content`.
///
/// Proto v3 removed direct buffer content RPCs; content arrives via projections.
/// This stub allows call sites to compile until the server-side emitter lands.
#[derive(Debug, Default)]
pub struct BufferContentStub {
    /// Always empty until `text.buffer_lines` projection is wired server-side.
    pub lines: Vec<String>,
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

/// gRPC v3 client for TUI.
///
/// Wraps all service clients and provides a unified interface for TUI operations.
/// This is the primary client for gRPC v3 communication.
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

        Ok(Self {
            input: InputServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            state: StateServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            buffer: BufferServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            notification: NotificationServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            server: ServerServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            editor: EditorServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            module: ModuleServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            presence: PresenceServiceClient::new(channel)
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
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
    pub async fn send_keys(&mut self, keys: &str) -> Result<SendInputResponse, TuiGrpcError> {
        let request = self.make_request(SendInputRequest {
            payload: keys.as_bytes().to_vec(),
            window_id: None,
            timestamp_ns: 0,
        });
        let response = self.input.send_input(request).await?;
        Ok(response.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // State Service
    // ─────────────────────────────────────────────────────────────────────────

    /// Get projections for the given tags.
    ///
    /// Returns the current projection entries for the specified tags.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn get_projections(
        &mut self,
        tags: &[&str],
    ) -> Result<Vec<ProjectionEntry>, TuiGrpcError> {
        let req = self.make_request(GetProjectionsRequest {
            tags: tags.iter().map(|s| (*s).to_string()).collect(),
            client_id: 0,
        });
        let resp = self.state.get_projections(req).await?.into_inner();
        Ok(resp.projections)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Panic Methods (#479: Fail Loud)
    // ─────────────────────────────────────────────────────────────────────────

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

    /// Get buffer content lines.
    ///
    /// # Note
    ///
    /// In proto v3, direct buffer content access via RPC was removed.
    /// Content is now expected to arrive via `ProjectionUpdated` with
    /// `tag == "text.buffer_lines"`. Until the server-side emitter lands,
    /// this method returns an empty line list so callers compile and run.
    ///
    /// TODO: Replace with projection-based fetch once server wires
    /// `text.buffer_lines` emitter — see `tmp/deferral-draft-buffer-lines-projection.md`.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - Optional buffer ID (unused pending projection emitter).
    /// * `start_line` - Optional start line (unused pending projection emitter).
    /// * `end_line` - Optional end line (unused pending projection emitter).
    pub async fn get_buffer_content(
        &mut self,
        _buffer_id: Option<u64>,
        _start_line: Option<u64>,
        _end_line: Option<u64>,
    ) -> Result<BufferContentStub, TuiGrpcError> {
        // Proto v3: buffer content is served via projections, not typed RPCs.
        // Return an empty stub so callers remain functional without content.
        tracing::trace!("get_buffer_content: no-op stub — server-side projection emitter pending");
        Ok(BufferContentStub { lines: Vec::new() })
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

    /// Notify the server of a surface change (replaces v2 `resize`).
    ///
    /// Encodes the cell-grid dimensions as a `SurfaceDescriptorProto` using the
    /// `CellGridCodec` body format (kind=0x0001, body=[width_be:u32, height_be:u32]).
    ///
    /// # Arguments
    ///
    /// * `width` - New viewport width in cells.
    /// * `height` - New viewport height in cells.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    pub async fn surface_changed(&mut self, width: u32, height: u32) -> Result<(), TuiGrpcError> {
        // Encode CellGridSurface: kind=0x0001, body=[width_be:u32, height_be:u32]
        let mut body = Vec::with_capacity(8);
        body.extend_from_slice(&width.to_be_bytes());
        body.extend_from_slice(&height.to_be_bytes());
        let surface = SurfaceDescriptorProto { kind: 0x0001, body };
        let request = self.make_request(SurfaceChangedRequest {
            surface: Some(surface),
        });
        let _response = self.editor.surface_changed(request).await?;
        Ok(())
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
            surface: None,
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
