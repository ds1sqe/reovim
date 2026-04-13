//! `DebugService` gRPC implementation.
//!
//! Provides debug, logging, and client-targeting operations for v2 protocol.
//!
//! # Architecture
//!
//! The CLI is **stateless** — it never joins as a client. All client-targeting
//! operations go through `DebugService`, which skips authentication and uses
//! `target_client_id` from the request body directly.
//!
//! # Features
//!
//! - `log_tail`: Get recent log entries from the server ring buffer
//! - `log_level`: Get/set the current log level
//! - `debug_send_keys`: Send keys to a specific client (no auth)
//! - `debug_capture`: Capture a client's screen via TUI relay (no auth)
//! - `debug_get_mode`: Get a client's editor mode (no auth)
//! - `debug_get_cursor`: Get a client's cursor position (no auth)
//! - `debug_list_clients`: List connected clients (no auth)

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]
// gRPC protocol uses u64 for IDs, but internally we use usize. On 64-bit
// platforms (our target), these are equivalent. Truncation on 32-bit is acceptable.
#![allow(clippy::cast_possible_truncation)]

use std::sync::Arc;

use {
    reovim_protocol::v2::{
        CaptureRequestPayload, DebugCaptureRequest, DebugCaptureResponse, DebugExtensionInfo,
        DebugGetCursorRequest, DebugGetCursorResponse, DebugGetExtensionStateRequest,
        DebugGetExtensionStateResponse, DebugGetModeRequest, DebugGetModeResponse,
        DebugListClientsRequest, DebugListClientsResponse, DebugListExtensionsRequest,
        DebugListExtensionsResponse, DebugSendKeysRequest, DebugSendKeysResponse, LogEntry,
        LogLevelRequest, LogLevelResponse, LogTailRequest, LogTailResponse, Notification, Position,
        SendKeysRequest, debug_service_server::DebugService, input_service_server::InputService,
        notification::Payload,
    },
    tonic::{Request, Response, Status},
};

use crate::{
    debug::try_debug_ring,
    grpc::{InputServiceImpl, presence::to_proto_client_info},
    session::{
        ClientId, Session, SessionId, SessionRegistry,
        capture::{CaptureError, wait_for_capture},
    },
};

use reovim_subsys_session::bridges::{BridgeRegistry, ExtensionScope};

/// Get the global debug ring buffer, returning `Status::unavailable` if not initialized.
///
/// The ring buffer is initialized once during server startup via `init_debug_ring()`.
/// In unit tests the initialization order is non-deterministic (`OnceLock`), so this
/// path cannot be reliably covered.
#[cfg_attr(coverage_nightly, coverage(off))]
fn require_debug_ring() -> Result<&'static crate::debug::DebugRingBuffer, Status> {
    try_debug_ring().ok_or_else(|| Status::unavailable("Debug ring buffer not initialized"))
}

/// Convert [`CaptureError`] to gRPC [`Status`].
fn capture_error_to_status(err: CaptureError) -> Status {
    match err {
        CaptureError::NoTuiClient => Status::failed_precondition(err.to_string()),
        CaptureError::Timeout => Status::deadline_exceeded(err.to_string()),
        CaptureError::Disconnected => Status::unavailable(err.to_string()),
        CaptureError::InvalidResponse(msg) => Status::internal(msg),
    }
}

/// gRPC `DebugService` implementation.
///
/// Bridges v2 protocol debug requests to the server's debug infrastructure.
/// When constructed with [`with_sessions`](Self::with_sessions), also provides
/// client-targeting operations for the stateless CLI.
pub struct DebugServiceImpl {
    /// Session registry (None = log-only mode for existing tests).
    sessions: Option<Arc<SessionRegistry>>,
    /// Default session ID (None = log-only mode).
    default_session_id: Option<SessionId>,
    /// Extension bridge registry (None = log-only mode).
    bridges: Option<Arc<BridgeRegistry>>,
}

impl DebugServiceImpl {
    /// Create a log-only `DebugService` (no client-targeting capabilities).
    ///
    /// Used by existing tests that only need `log_tail` / `log_level`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sessions: None,
            default_session_id: None,
            bridges: None,
        }
    }

    /// Create a full `DebugService` with client-targeting capabilities.
    ///
    /// Used by the server router when all services are available.
    #[must_use]
    pub const fn with_sessions(
        sessions: Arc<SessionRegistry>,
        default_session_id: SessionId,
        bridges: Arc<BridgeRegistry>,
    ) -> Self {
        Self {
            sessions: Some(sessions),
            default_session_id: Some(default_session_id),
            bridges: Some(bridges),
        }
    }

    /// Get the default session, or return `Unavailable` if not configured.
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        let sessions = self
            .sessions
            .as_ref()
            .ok_or_else(|| Status::unavailable("DebugService not configured with sessions"))?;
        let session_id = self
            .default_session_id
            .as_ref()
            .expect("session_id set with sessions");
        sessions
            .get(session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }

    /// Resolve a `target_client_id` (u64) to a `ClientId`.
    ///
    /// Returns `InvalidArgument` if `target_client_id` is 0 (reserved).
    fn resolve_target(target_client_id: u64) -> Result<ClientId, Status> {
        if target_client_id == 0 {
            return Err(Status::invalid_argument(
                "target_client_id=0 is reserved; specify the target client ID",
            ));
        }
        Ok(ClientId::new(target_client_id as usize))
    }
}

impl Default for DebugServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl DebugService for DebugServiceImpl {
    // ── Logging ──────────────────────────────────────────────────────────────

    /// Get recent log entries from the server ring buffer.
    async fn log_tail(
        &self,
        request: Request<LogTailRequest>,
    ) -> Result<Response<LogTailResponse>, Status> {
        let req = request.into_inner();
        let count = if req.count == 0 {
            50
        } else {
            req.count as usize
        };

        let ring = require_debug_ring()?;

        let entries = ring.tail(count);

        // Convert to proto format with optional filtering
        let proto_entries: Vec<LogEntry> = entries
            .into_iter()
            .filter_map(|e| {
                // Level filter
                if let Some(ref level) = req.level
                    && !e.level.to_string().eq_ignore_ascii_case(level)
                {
                    return None;
                }
                // Target filter
                if let Some(ref target) = req.target
                    && !e.target.contains(target)
                {
                    return None;
                }
                // Grep filter
                if let Some(ref grep) = req.grep
                    && !e.message.to_lowercase().contains(&grep.to_lowercase())
                {
                    return None;
                }
                // Take ownership of strings since we're consuming `e`
                Some(LogEntry {
                    seq: e.seq,
                    timestamp_us: e.timestamp_us,
                    level: e.level.to_string(),
                    target: e.target,
                    message: e.message,
                })
            })
            .collect();

        Ok(Response::new(LogTailResponse {
            entries: proto_entries,
        }))
    }

    /// Get or set the current log level.
    async fn log_level(
        &self,
        _request: Request<LogLevelRequest>,
    ) -> Result<Response<LogLevelResponse>, Status> {
        // TODO: Implement dynamic log level control
        Ok(Response::new(LogLevelResponse {
            level: "info".to_string(),
        }))
    }

    // ── Client-targeting operations ──────────────────────────────────────────

    /// Send keys to a specific client's state (direct, no relay).
    ///
    /// Constructs an authenticated `InputService::SendKeys` request with the
    /// target's `ClientId` injected into extensions, then delegates to the
    /// standard key processing pipeline. All server-side state (resolvers,
    /// mode stack, commands) is accessed directly.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn debug_send_keys(
        &self,
        request: Request<DebugSendKeysRequest>,
    ) -> Result<Response<DebugSendKeysResponse>, Status> {
        let req = request.into_inner();
        let client_id = Self::resolve_target(req.target_client_id)?;

        let sessions = self
            .sessions
            .as_ref()
            .ok_or_else(|| Status::unavailable("DebugService not configured with sessions"))?;
        let session_id = self
            .default_session_id
            .as_ref()
            .expect("session_id set with sessions");
        let bridges = self
            .bridges
            .as_ref()
            .ok_or_else(|| Status::unavailable("DebugService not configured with bridges"))?;

        // Build an InputServiceImpl and delegate with ClientId in extensions
        let input_service =
            InputServiceImpl::new(Arc::clone(sessions), session_id.clone(), Arc::clone(bridges));

        let mut send_request = Request::new(SendKeysRequest { keys: req.keys });
        send_request.extensions_mut().insert(client_id);

        let response = InputService::send_keys(&input_service, send_request).await?;
        let inner = response.into_inner();

        Ok(Response::new(DebugSendKeysResponse {
            ok: inner.ok,
            status: inner.status,
        }))
    }

    /// Capture a client's screen content via TUI relay.
    ///
    /// Same relay flow as `StateService::get_screen_content`:
    /// 1. Create pending capture request
    /// 2. Emit `capture_request` notification to target TUI
    /// 3. Wait for TUI response
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn debug_capture(
        &self,
        request: Request<DebugCaptureRequest>,
    ) -> Result<Response<DebugCaptureResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // Validate format
        let format = if req.format.is_empty() {
            "raw_ansi".to_string()
        } else {
            match req.format.as_str() {
                "plain_text" | "raw_ansi" | "cell_grid" => req.format,
                _ => {
                    return Err(Status::invalid_argument(format!(
                        "Invalid format '{}'. Use 'plain_text', 'raw_ansi', or 'cell_grid'",
                        req.format
                    )));
                }
            }
        };

        let target_client_id = req.target_client_id;

        // Create pending capture request
        let (request_id, rx) = session.capture_tracker().create_pending();

        // Build and emit capture_request notification
        let notification = Notification {
            event_type: "capture_request".to_string(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before UNIX_EPOCH")
                .as_millis() as u64,
            payload: Some(Payload::CaptureRequest(CaptureRequestPayload {
                request_id,
                format: format.clone(),
                target_client_id,
            })),
        };

        session.emit_notification(notification);
        tracing::debug!(request_id, target_client_id, format, "Sent debug capture_request");

        // Wait for TUI response with timeout
        let result = wait_for_capture(rx).await.map_err(|e: CaptureError| {
            session.capture_tracker().cancel(request_id);
            capture_error_to_status(e)
        })?;

        Ok(Response::new(DebugCaptureResponse {
            width: result.width as u32,
            height: result.height as u32,
            format: result.format,
            content: result.content,
        }))
    }

    /// Get a client's current editor mode (direct server-side lookup).
    async fn debug_get_mode(
        &self,
        request: Request<DebugGetModeRequest>,
    ) -> Result<Response<DebugGetModeResponse>, Status> {
        let req = request.into_inner();
        let client_id = Self::resolve_target(req.target_client_id)?;
        let session = self.get_session()?;

        let mode = session.client_current_mode(client_id).ok_or_else(|| {
            Status::not_found(format!("Client {} not found", req.target_client_id))
        })?;

        let name = mode.name().to_string();
        let display = name.to_uppercase();
        let is_insert = name.contains("insert") || name.contains("cmdline");

        Ok(Response::new(DebugGetModeResponse {
            name,
            display,
            is_insert,
        }))
    }

    /// Get a client's cursor position (direct server-side lookup).
    async fn debug_get_cursor(
        &self,
        request: Request<DebugGetCursorRequest>,
    ) -> Result<Response<DebugGetCursorResponse>, Status> {
        let req = request.into_inner();
        let client_id = Self::resolve_target(req.target_client_id)?;
        let session = self.get_session()?;

        let state = session.clients().client_state(client_id).ok_or_else(|| {
            Status::not_found(format!("Client {} not found", req.target_client_id))
        })?;

        let window = state
            .windows
            .active()
            .ok_or_else(|| Status::not_found("No active window"))?;

        let cursor = &window.cursor;
        Ok(Response::new(DebugGetCursorResponse {
            window_id: window.id.as_usize() as u64,
            position: Some(Position {
                line: cursor.line as u64,
                column: cursor.column as u64,
            }),
        }))
    }

    /// List connected clients (read-only, no auth).
    async fn debug_list_clients(
        &self,
        _request: Request<DebugListClientsRequest>,
    ) -> Result<Response<DebugListClientsResponse>, Status> {
        let session = self.get_session()?;

        let clients = session
            .clients()
            .with_clients(|c| c.values().map(to_proto_client_info).collect());

        Ok(Response::new(DebugListClientsResponse { clients }))
    }

    // ── Extension state queries ─────────────────────────────────────────────

    /// Query extension state for a specific client (direct, no auth).
    async fn debug_get_extension_state(
        &self,
        request: Request<DebugGetExtensionStateRequest>,
    ) -> Result<Response<DebugGetExtensionStateResponse>, Status> {
        let req = request.into_inner();
        let client_id = Self::resolve_target(req.target_client_id)?;
        let session = self.get_session()?;

        let bridges = self
            .bridges
            .as_ref()
            .ok_or_else(|| Status::unavailable("DebugService not configured with bridges"))?;

        let bridge = bridges
            .get(&req.kind)
            .ok_or_else(|| Status::not_found(format!("Unknown extension kind: {}", req.kind)))?;

        let (active, snapshot) = match bridge.scope() {
            ExtensionScope::Client => session
                .clients()
                .with_client_extensions(client_id, |extensions| {
                    let active = bridge.is_active(extensions);
                    let snap = bridge.snapshot(extensions);
                    (active, snap)
                })
                .unwrap_or((false, None)),
            ExtensionScope::Shared => {
                session
                    .with_state(|state| {
                        let extensions = &state.app.extensions;
                        let active = bridge.is_active(extensions);
                        let snap = bridge.snapshot(extensions);
                        (active, snap)
                    })
                    .await
            }
        };

        Ok(Response::new(DebugGetExtensionStateResponse {
            active,
            data: snapshot.map_or_else(String::new, |v| v.to_string()),
        }))
    }

    /// List all registered extensions (read-only, no auth).
    async fn debug_list_extensions(
        &self,
        _request: Request<DebugListExtensionsRequest>,
    ) -> Result<Response<DebugListExtensionsResponse>, Status> {
        let bridges = self
            .bridges
            .as_ref()
            .ok_or_else(|| Status::unavailable("DebugService not configured with bridges"))?;

        let extensions = bridges
            .kinds()
            .into_iter()
            .map(|kind| {
                let scope = bridges.get(kind).map_or("unknown", |b| match b.scope() {
                    ExtensionScope::Client => "client",
                    ExtensionScope::Shared => "shared",
                });
                DebugExtensionInfo {
                    kind: kind.to_string(),
                    scope: scope.to_string(),
                }
            })
            .collect();

        Ok(Response::new(DebugListExtensionsResponse { extensions }))
    }
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;
