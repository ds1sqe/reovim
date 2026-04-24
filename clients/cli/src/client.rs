//! gRPC v2 client wrapper.
//!
//! Provides a unified client interface to all gRPC v2 services.

use {
    reovim_protocol::v3::{
        DebugCaptureRequest, DebugCaptureResponse, DebugGetExtensionStateRequest,
        DebugGetExtensionStateResponse, DebugGetProjectionsRequest, DebugGetProjectionsResponse,
        DebugListClientsRequest, DebugListClientsResponse, DebugListExtensionsRequest,
        DebugListExtensionsResponse, DebugSendInputRequest, DebugSendInputResponse,
        DebugStreamClientMsg, DebugStreamServerMsg, GetProjectionsRequest, GetProjectionsResponse,
        GetRegistersRequest, GetRegistersResponse, GetScreenContentRequest,
        GetScreenContentResponse, InfoRequest, InfoResponse, JoinRequest, JoinResponse,
        LeaveRequest, LeaveResponse, ListBuffersRequest, ListBuffersResponse, ListClientsRequest,
        ListClientsResponse, ListModulesRequest, ListModulesResponse, LogTailRequest,
        LogTailResponse, PingRequest, PingResponse, SendInputRequest, SendInputResponse,
        SetSyncModeRequest, SetSyncModeResponse, UpdatePresenceRequest, UpdatePresenceResponse,
        buffer_service_client::BufferServiceClient,
        client_debug_service_client::ClientDebugServiceClient,
        debug_service_client::DebugServiceClient, input_service_client::InputServiceClient,
        module_service_client::ModuleServiceClient, presence_service_client::PresenceServiceClient,
        server_service_client::ServerServiceClient, state_service_client::StateServiceClient,
    },
    tokio_stream::Stream,
    tonic::{Request, Streaming, transport::Channel},
};

/// Error type for gRPC client operations.
#[derive(Debug)]
pub enum GrpcClientError {
    /// Failed to connect to the server.
    ConnectionFailed(String),
    /// gRPC call failed.
    GrpcError(tonic::Status),
    /// Invalid CLI argument combination.
    InvalidArgument(String),
    /// Local workflow or filesystem operation failed.
    OperationFailed(String),
    /// Web capture script failed.
    CaptureError(String),
}

impl std::fmt::Display for GrpcClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::GrpcError(status) => write!(f, "gRPC error: {status}"),
            Self::InvalidArgument(msg) => write!(f, "Invalid argument: {msg}"),
            Self::OperationFailed(msg) => write!(f, "Operation failed: {msg}"),
            Self::CaptureError(msg) => write!(f, "Capture error: {msg}"),
        }
    }
}

impl std::error::Error for GrpcClientError {}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<tonic::Status> for GrpcClientError {
    fn from(status: tonic::Status) -> Self {
        Self::GrpcError(status)
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<tonic::transport::Error> for GrpcClientError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::ConnectionFailed(err.to_string())
    }
}

/// Maximum gRPC message size (encoding and decoding) in bytes.
///
/// Matches the server-side limit. Required for large buffer content
/// such as hex dumps of binary files.
const GRPC_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// gRPC v2 client for interacting with the reovim server.
///
/// Wraps all service clients (Input, State, Buffer, Server, Presence, Debug) and provides
/// a unified interface.
///
/// # Phase #479: Client ID Management
///
/// The client auto-joins the presence session on first use and stores the assigned
/// `client_id`. All subsequent calls use this ID for per-client state isolation.
pub struct GrpcClient {
    input: InputServiceClient<Channel>,
    state: StateServiceClient<Channel>,
    buffer: BufferServiceClient<Channel>,
    server: ServerServiceClient<Channel>,
    module: ModuleServiceClient<Channel>,
    presence: PresenceServiceClient<Channel>,
    debug: DebugServiceClient<Channel>,
    /// Driver-owned debug-surface client (#770). Separate from the
    /// legacy `debug` field — different service.
    client_debug: ClientDebugServiceClient<Channel>,
    /// Server address for error messages.
    address: String,
    /// Client ID assigned by server (None until joined).
    client_id: Option<u64>,
    /// Session token for token-based authentication (#483).
    ///
    /// Received from `Join()` response, sent as `x-reovim-token` metadata
    /// header on every subsequent gRPC request.
    session_token: Option<String>,
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn connect(addr: &str) -> Result<Self, GrpcClientError> {
        // Build the endpoint URL
        let url = format!("http://{addr}");
        let channel = Channel::from_shared(url)
            .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
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
            server: ServerServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            module: ModuleServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            presence: PresenceServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            debug: DebugServiceClient::new(channel.clone())
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            client_debug: ClientDebugServiceClient::new(channel)
                .max_decoding_message_size(GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(GRPC_MAX_MESSAGE_SIZE),
            address: addr.to_string(),
            client_id: None,
            session_token: None,
        })
    }

    /// Wrap a proto message in a `tonic::Request` with session token metadata (#483).
    ///
    /// If a session token has been stored (from `Join()`), injects it as the
    /// `x-reovim-token` gRPC metadata header. The server interceptor resolves
    /// this token to the caller's `ClientId`.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn make_request<T>(&self, body: T) -> Request<T> {
        let mut request = Request::new(body);
        if let Some(ref token) = self.session_token {
            request
                .metadata_mut()
                .insert("x-reovim-token", token.parse().expect("session token is valid ASCII"));
        }
        request
    }

    /// Ensure the client has joined the presence session.
    ///
    /// If not already joined, calls `presence_join()` to get a client ID.
    /// Panics if joining fails - CLI cannot operate without a valid client ID.
    ///
    /// # Phase #479: Auto-join for per-client state
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn ensure_joined(&mut self) -> u64 {
        if let Some(id) = self.client_id {
            return id;
        }

        match self.presence_join("cli", "CLI").await {
            Ok(resp) => {
                self.client_id = Some(resp.client_id);
                resp.client_id
            }
            Err(e) => panic!(
                "FATAL: Failed to join presence session.\n\
                 Server: {}\n\
                 Error: {e}\n\
                 Ensure server is running and accepts connections.",
                self.address
            ),
        }
    }

    /// Handle gRPC errors with panic vs retry policy.
    ///
    /// # Phase #479: Panic on client bugs, panic on transient (for now)
    ///
    /// This function can be used by command handlers that want to panic on
    /// errors rather than propagating them to the caller.
    #[allow(dead_code)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handle_grpc_error(e: &tonic::Status, operation: &str) -> ! {
        use tonic::Code;

        match e.code() {
            // PANIC - Client bug or misconfiguration
            Code::NotFound
            | Code::InvalidArgument
            | Code::PermissionDenied
            | Code::FailedPrecondition
            | Code::Internal
            | Code::Unimplemented => {
                panic!(
                    "FATAL: {operation} failed\n\
                     Code: {:?}\n\
                     Message: {}\n\
                     This indicates a client bug or misconfiguration.",
                    e.code(),
                    e.message()
                );
            }
            // RETRY - Transient issues (for now, panic - retry logic can be added later)
            Code::Unavailable
            | Code::ResourceExhausted
            | Code::DeadlineExceeded
            | Code::Aborted => {
                panic!(
                    "FATAL: {operation} failed (transient)\n\
                     Code: {:?}\n\
                     Message: {}\n\
                     Server may be temporarily unavailable.",
                    e.code(),
                    e.message()
                );
            }
            // Unknown - log and panic
            _ => {
                panic!(
                    "FATAL: {operation} failed (unknown)\n\
                     Code: {:?}\n\
                     Message: {}",
                    e.code(),
                    e.message()
                );
            }
        }
    }

    /// Send keys to the editor.
    ///
    /// # Arguments
    ///
    /// * `keys` - Keys in vim notation (e.g., `iHello<Esc>`).
    ///
    /// # Returns
    ///
    /// The response containing success status and key status.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    ///
    /// # Phase #479: Auto-join and panic on error
    ///
    /// The client auto-joins the presence session on first call.
    /// Panics if joining fails or if the server returns a fatal error.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn send_input(&mut self, keys: &str) -> Result<SendInputResponse, GrpcClientError> {
        self.ensure_joined().await;
        let request = self.make_request(SendInputRequest {
            payload: keys.as_bytes().to_vec(),
            window_id: None,
            timestamp_ns: 0,
        });
        let response = self.input.send_input(request).await?;
        Ok(response.into_inner())
    }

    /// Backward-compatible alias for `send_input`.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn send_keys(&mut self, keys: &str) -> Result<SendInputResponse, GrpcClientError> {
        self.send_input(keys).await
    }

    /// Get the current editor mode.
    ///
    /// # Phase #479: Auto-join and per-client state
    /// Get domain-neutral projection state for this client (#753).
    ///
    /// Replaces `get_mode` and `get_cursor` — domain state is now queried
    /// via projection tags (e.g., "text.mode", "text.cursor").
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn get_projections(
        &mut self,
        client_id: u64,
        tags: Vec<String>,
    ) -> Result<GetProjectionsResponse, GrpcClientError> {
        let request = self.make_request(GetProjectionsRequest { client_id, tags });
        let response = self.state.get_projections(request).await?;
        Ok(response.into_inner())
    }

    /// List all open buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn list_buffers(&mut self) -> Result<ListBuffersResponse, GrpcClientError> {
        let request = self.make_request(ListBuffersRequest {});
        let response = self.buffer.list(request).await?;
        Ok(response.into_inner())
    }

    /// Ping the server (health check).
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn ping(&mut self) -> Result<PingResponse, GrpcClientError> {
        let request = self.make_request(PingRequest {});
        let response = self.server.ping(request).await?;
        Ok(response.into_inner())
    }

    /// Get server info.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn info(&mut self) -> Result<InfoResponse, GrpcClientError> {
        let request = self.make_request(InfoRequest {});
        let response = self.server.info(request).await?;
        Ok(response.into_inner())
    }

    /// List loaded modules via `ModuleService`.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn module_list(&mut self) -> Result<ListModulesResponse, GrpcClientError> {
        let request = self.make_request(ListModulesRequest {});
        let response = self.module.list(request).await?;
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn get_registers(
        &mut self,
        names: Vec<String>,
    ) -> Result<GetRegistersResponse, GrpcClientError> {
        let request = self.make_request(GetRegistersRequest {
            names,
            client_id: 0, // 0 = self (authenticated client)
        });
        let response = self.state.get_registers(request).await?;
        Ok(response.into_inner())
    }

    /// Get screen content via TUI capture relay.
    ///
    /// Requests a screen capture from a specific TUI client via the server.
    ///
    /// # Arguments
    ///
    /// * `client_id` - Target client ID to capture from
    /// * `format` - Capture format: `plain_text`, `raw_ansi`, or `cell_grid`
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails, no TUI is connected, or capture times out.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn get_screen_content(
        &mut self,
        client_id: u64,
        format: &str,
    ) -> Result<GetScreenContentResponse, GrpcClientError> {
        // client_id here is a TARGET (which TUI to capture), not caller identity
        let request = self.make_request(GetScreenContentRequest {
            client_id,
            format: format.to_string(),
        });
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn presence_join(
        &mut self,
        client_type: &str,
        display_name: &str,
    ) -> Result<JoinResponse, GrpcClientError> {
        let request = JoinRequest {
            client_type: client_type.to_string(),
            display_name: display_name.to_string(),
            surface: None,
        };
        let response = self.presence.join(request).await?.into_inner();
        // Store client ID and session token for subsequent requests (#483)
        self.client_id = Some(response.client_id);
        if !response.session_token.is_empty() {
            self.session_token = Some(response.session_token.clone());
        }
        Ok(response)
    }

    /// Leave the presence session.
    ///
    /// # Phase 5 (#483): Token identifies the client — no `client_id` parameter.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn presence_leave(&mut self) -> Result<LeaveResponse, GrpcClientError> {
        let request = self.make_request(LeaveRequest {});
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn presence_list(&mut self) -> Result<ListClientsResponse, GrpcClientError> {
        let request = self.make_request(ListClientsRequest {});
        let response = self.presence.list_clients(request).await?;
        Ok(response.into_inner())
    }

    /// Update this client's presence state.
    ///
    /// # Phase 5 (#483): Token identifies the client — no `client_id` parameter.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn presence_update(
        &mut self,
        buffer_id: Option<u64>,
        _mode: Option<String>,
    ) -> Result<UpdatePresenceResponse, GrpcClientError> {
        let request = self.make_request(UpdatePresenceRequest {
            buffer_id,
            viewport_state: None,
            spatial_state: None,
        });
        let response = self.presence.update_presence(request).await?;
        Ok(response.into_inner())
    }

    /// Set sync mode for this client.
    ///
    /// # Phase 5 (#483): Token identifies the client — no `client_id` parameter.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn presence_set_sync_mode(
        &mut self,
        sync_mode: i32,
        follow_target: Option<u64>,
    ) -> Result<SetSyncModeResponse, GrpcClientError> {
        let request = self.make_request(SetSyncModeRequest {
            mode: sync_mode,
            follow_target,
        });
        let response = self.presence.set_sync_mode(request).await?;
        Ok(response.into_inner())
    }

    // =========================================================================
    // Debug Service Methods (Phase 17, #481)
    // =========================================================================

    /// Get recent log entries from the server ring buffer.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of entries to retrieve (default: 50).
    /// * `level` - Optional level filter (trace, debug, info, warn, error).
    /// * `target` - Optional target module filter (contains match).
    /// * `grep` - Optional message filter (case-insensitive contains).
    ///
    /// # Returns
    ///
    /// The response containing log entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn log_tail(
        &mut self,
        count: u32,
        level: Option<String>,
        target: Option<String>,
        grep: Option<String>,
    ) -> Result<LogTailResponse, GrpcClientError> {
        let request = self.make_request(LogTailRequest {
            count,
            level,
            target,
            grep,
        });
        let response = self.debug.log_tail(request).await?;
        Ok(response.into_inner())
    }

    // =========================================================================
    // Debug Client-Targeting Methods (#468)
    //
    // CLI is stateless — no join, no token, no presence.
    // These methods target specific connected clients (TUI/Web) by ID.
    // =========================================================================

    /// Send keys to a specific client's state via `DebugService`.
    ///
    /// No auth required — CLI targets the client by ID directly.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails or target client doesn't exist.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_send_input(
        &mut self,
        keys: &str,
        target_client_id: u64,
    ) -> Result<DebugSendInputResponse, GrpcClientError> {
        let request = Request::new(DebugSendInputRequest {
            payload: keys.as_bytes().to_vec(),
            target_client_id,
        });
        let response = self.debug.debug_send_input(request).await?;
        Ok(response.into_inner())
    }

    /// Backward-compatible alias.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails or target client doesn't exist.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_send_keys(
        &mut self,
        keys: &str,
        target_client_id: u64,
    ) -> Result<DebugSendInputResponse, GrpcClientError> {
        self.debug_send_input(keys, target_client_id).await
    }

    /// Capture a specific client's screen content via `DebugService`.
    ///
    /// No auth required — CLI targets the client by ID directly.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails, client doesn't exist, or capture times out.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_capture(
        &mut self,
        target_client_id: u64,
        format: &str,
    ) -> Result<DebugCaptureResponse, GrpcClientError> {
        let request = Request::new(DebugCaptureRequest {
            target_client_id,
            format: format.to_string(),
        });
        let response = self.debug.debug_capture(request).await?;
        Ok(response.into_inner())
    }

    /// Get a specific client's cursor position via `DebugService`.
    ///
    /// No auth required — CLI targets the client by ID directly.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails or target client doesn't exist.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_get_projections(
        &mut self,
        target_client_id: u64,
        tags: Vec<String>,
    ) -> Result<DebugGetProjectionsResponse, GrpcClientError> {
        let request = Request::new(DebugGetProjectionsRequest {
            target_client_id,
            tags,
        });
        let response = self.debug.debug_get_projections(request).await?;
        Ok(response.into_inner())
    }

    /// List connected clients via `DebugService`.
    ///
    /// No auth required — read-only debug query.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_list_clients(
        &mut self,
    ) -> Result<DebugListClientsResponse, GrpcClientError> {
        let request = Request::new(DebugListClientsRequest {});
        let response = self.debug.debug_list_clients(request).await?;
        Ok(response.into_inner())
    }

    /// Query extension state for a specific client via `DebugService`.
    ///
    /// No auth required — CLI targets the client by ID directly.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails or extension kind is unknown.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_get_extension_state(
        &mut self,
        kind: &str,
        target_client_id: u64,
    ) -> Result<DebugGetExtensionStateResponse, GrpcClientError> {
        let request = Request::new(DebugGetExtensionStateRequest {
            kind: kind.to_string(),
            target_client_id,
        });
        let response = self.debug.debug_get_extension_state(request).await?;
        Ok(response.into_inner())
    }

    /// List all registered extensions via `DebugService`.
    ///
    /// No auth required — read-only debug query.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC call fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_list_extensions(
        &mut self,
    ) -> Result<DebugListExtensionsResponse, GrpcClientError> {
        let request = Request::new(DebugListExtensionsRequest {});
        let response = self.debug.debug_list_extensions(request).await?;
        Ok(response.into_inner())
    }

    /// Open a bidirectional `ClientDebugService::DebugStream` (#770).
    ///
    /// `requests` is the outbound client message stream; the returned
    /// `Streaming<DebugStreamServerMsg>` yields server responses until
    /// EOS or error.
    ///
    /// # Errors
    /// Returns a `GrpcClientError::GrpcError` if the stream cannot be
    /// opened (e.g. service not registered on the server).
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn debug_stream(
        &mut self,
        requests: impl Stream<Item = DebugStreamClientMsg> + Send + 'static,
    ) -> Result<Streaming<DebugStreamServerMsg>, GrpcClientError> {
        let response = self.client_debug.debug_stream(requests).await?;
        Ok(response.into_inner())
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
