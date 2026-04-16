//! `StateService` gRPC implementation.
//!
//! Provides editor state queries for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_protocol::v2::{
        CaptureRequestPayload, GetLayoutRequest, GetLayoutResponse, GetOptionsRequest,
        GetOptionsResponse, GetProjectionsRequest, GetProjectionsResponse, GetRegistersRequest,
        GetRegistersResponse, GetScreenContentRequest, GetScreenContentResponse,
        GetVisibleLinesRequest, GetVisibleLinesResponse, Notification, RegisterEntry,
        SplitDirection, SubmitCaptureRequest, SubmitCaptureResponseReply, TabPageInfo, WindowLeaf,
        WindowNode, WindowRect, WindowSplit, notification::Payload,
        state_service_server::StateService, window_node::Node,
    },
    tonic::{Request, Response, Status},
};

use crate::{
    grpc::auth::resolve_target_client_id,
    session::{
        CaptureError, CaptureResult, ClientEventType, ClientId, Session, SessionId,
        SessionRegistry, wait_for_capture,
    },
};

/// Convert a `CaptureError` into a tonic `Status`.
fn capture_error_to_status(e: CaptureError) -> Status {
    match e {
        CaptureError::NoTuiClient => Status::unavailable(e.to_string()),
        CaptureError::Timeout => Status::deadline_exceeded(e.to_string()),
        CaptureError::Disconnected => Status::aborted(e.to_string()),
        CaptureError::InvalidResponse(msg) => Status::internal(msg),
    }
}

/// Convert a compositor placement to a proto `WindowLeaf`.
///
/// buffer_id is domain-owned (#753 E3) — not available here.
#[allow(clippy::cast_possible_truncation)]
fn placement_to_leaf(
    placement: &reovim_subsys_layout::WindowPlacement,
    buffer_id: Option<u64>,
) -> WindowLeaf {
    WindowLeaf {
        window_id: placement.window_id.as_usize() as u64,
        buffer_id,
        rect: Some(WindowRect {
            x: u64::from(placement.bounds.x),
            y: u64::from(placement.bounds.y),
            width: u64::from(placement.bounds.width),
            height: u64::from(placement.bounds.height),
        }),
    }
}

/// Convert a kernel `OptionValue` to a proto `OptionValue`.
fn kernel_to_proto_option(
    value: &reovim_kernel::api::v1::OptionValue,
) -> reovim_protocol::v2::OptionValue {
    use reovim_protocol::v2::option_value::Value;

    let proto_value = match value {
        reovim_kernel::api::v1::OptionValue::Bool(b) => Some(Value::BoolValue(*b)),
        reovim_kernel::api::v1::OptionValue::Integer(i) => Some(Value::IntValue(*i)),
        reovim_kernel::api::v1::OptionValue::String(s) => Some(Value::StringValue(s.clone())),
        reovim_kernel::api::v1::OptionValue::Choice { value, .. } => {
            Some(Value::StringValue(value.clone()))
        }
    };

    reovim_protocol::v2::OptionValue { value: proto_value }
}

/// gRPC `StateService` implementation.
///
/// Bridges v2 protocol state queries to the session system.
/// Currently provides basic mode and cursor information.
pub struct StateServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
}

impl StateServiceImpl {
    /// Create a new `StateService` with access to the session registry.
    #[must_use]
    pub const fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
        }
    }

    /// Get the default session.
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl StateService for StateServiceImpl {
    /// Get domain-neutral projection state (#753).
    ///
    /// Replaces `GetMode`, `GetCursor`, `GetSelection`. Domain state is now
    /// queried via projection tags (e.g., "text.mode", "text.cursor").
    ///
    /// Stub: returns empty projections until the projection store query is wired.
    async fn get_projections(
        &self,
        request: Request<GetProjectionsRequest>,
    ) -> Result<Response<GetProjectionsResponse>, Status> {
        let _req = request.into_inner();
        self.get_session()?;
        // Stub: return empty projections until projection store query is wired.
        Ok(Response::new(GetProjectionsResponse {
            projections: vec![],
        }))
    }

    /// Get editor options.
    ///
    /// Returns the current values of requested options. If `names` is empty,
    /// returns all registered options. Values are resolved at global scope.
    ///
    /// # Errors
    ///
    /// - `NotFound`: No active session
    #[allow(clippy::cast_possible_truncation)]
    async fn get_options(
        &self,
        request: Request<GetOptionsRequest>,
    ) -> Result<Response<GetOptionsResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        let options = session
            .with_state(|state| {
                use reovim_kernel::api::v1::OptionScopeId;

                let registry = &state.app.kernel.options;
                let names: Vec<String> = if req.names.is_empty() {
                    registry.list_all()
                } else {
                    req.names
                        .iter()
                        .filter_map(|n| registry.resolve_name(n))
                        .collect()
                };

                let mut map = std::collections::HashMap::new();
                for name in &names {
                    if let Some(value) = registry.get(name, OptionScopeId::Global) {
                        map.insert(name.clone(), kernel_to_proto_option(&value));
                    }
                }
                map
            })
            .await;

        Ok(Response::new(GetOptionsResponse { options }))
    }

    /// Get window layout tree.
    ///
    /// Returns the current window layout as a tree structure. Each window
    /// is represented as a `WindowLeaf` with its buffer, position, and size.
    /// Multiple windows are wrapped in a `WindowSplit` container.
    ///
    /// # Layout Model
    ///
    /// For Phase 11, windows are stored in a flat list. The proto tree is
    /// constructed as:
    /// - 0 windows: empty response (no root)
    /// - 1 window: single `WindowLeaf`
    /// - N windows: `WindowSplit` with N leaf children (vertical split)
    ///
    /// Phase 12 (`LayoutService`) will add proper nested split support.
    ///
    /// # Phase #471: Per-client layout isolation
    ///
    /// Returns the layout from the client's per-client window layout.
    ///
    /// # Errors
    ///
    /// - `InvalidArgument`: `client_id=0` is reserved (like PID 1)
    /// - `NotFound`: Client with given ID not found in session
    #[allow(clippy::cast_possible_truncation)]
    async fn get_layout(
        &self,
        request: Request<GetLayoutRequest>,
    ) -> Result<Response<GetLayoutResponse>, Status> {
        // #483 Phase 5: Token for auth, body client_id for targeting
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_id = resolve_target_client_id(token_client_id, req.client_id)?;

        // Per-client state lookup - now required (no fallback to shared state)
        let state = session.clients().client_state(client_id).ok_or_else(|| {
            session.with_client_ring_buffer(client_id, |rb| {
                rb.log_event(
                    ClientEventType::Error,
                    format!("CLIENT_NOT_FOUND: get_layout client_id={}", req.client_id),
                );
            });
            Status::not_found(format!("Client {} not found", req.client_id))
        })?;

        // Build layout from per-client compositor (#474).
        // buffer_id and tabs are domain-owned (#753 E3) — return None/empty until E5/E6.
        let active_buffer = session
            .active_buffer_for_client(client_id)
            .map(|id| id.as_usize() as u64);

        let (root, focused_id) = if let Some(ref compositor) = state.compositor {
            let (tw, th) = state.terminal_size;
            let screen = reovim_subsys_layout::Rect::new(0, 0, tw, th);
            let composite = compositor.composite(screen);
            let focused_id = composite.focused.map(|id| id.as_usize() as u64);

            let root = match composite.placements.len() {
                0 => None,
                1 => Some(WindowNode {
                    node: Some(Node::Leaf(placement_to_leaf(
                        &composite.placements[0],
                        active_buffer,
                    ))),
                }),
                _ => {
                    let children: Vec<WindowNode> = composite
                        .placements
                        .iter()
                        .map(|p| WindowNode {
                            node: Some(Node::Leaf(placement_to_leaf(p, active_buffer))),
                        })
                        .collect();
                    Some(WindowNode {
                        node: Some(Node::Split(WindowSplit {
                            direction: SplitDirection::Vertical.into(),
                            children,
                        })),
                    })
                }
            };
            (root, focused_id)
        } else {
            (None, None)
        };

        // tabs: domain-owned — return empty (#753 E3)
        let active_tab_id = None;
        let tabs_info: Vec<TabPageInfo> = Vec::new();

        Ok(Response::new(GetLayoutResponse {
            root,
            focused_window_id: focused_id,
            active_tab_id,
            tabs: tabs_info,
        }))
    }

    /// Get visible line range for a window.
    ///
    /// Returns the first and last visible line indices based on the window's
    /// viewport scroll position and height.
    ///
    /// # Arguments
    ///
    /// - `window_id`: Optional window ID. If not specified, uses the focused window.
    /// - `client_id`: Required client ID for per-client viewport
    ///
    /// # Returns
    ///
    /// - `first_line`: First visible line (0-indexed, based on `scroll_top`)
    /// - `last_line`: Last visible line (0-indexed, `scroll_top` + height - 1)
    /// - `viewport_height`: Number of visible lines
    ///
    /// # Phase #471: Per-client viewport isolation
    ///
    /// Returns visible lines from the client's per-client window layout.
    ///
    /// # Errors
    ///
    /// - `InvalidArgument`: `client_id=0` is reserved (like PID 1)
    /// - `NotFound`: Client with given ID not found, or window not found
    #[allow(clippy::cast_possible_truncation)]
    async fn get_visible_lines(
        &self,
        request: Request<GetVisibleLinesRequest>,
    ) -> Result<Response<GetVisibleLinesResponse>, Status> {
        // #483 Phase 5: Token for auth, body client_id for targeting
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let requested_window_id = req.window_id;
        let session = self.get_session()?;

        let client_id = resolve_target_client_id(token_client_id, req.client_id)?;

        // Per-client state lookup
        let state = session.clients().client_state(client_id).ok_or_else(|| {
            session.with_client_ring_buffer(client_id, |rb| {
                rb.log_event(
                    ClientEventType::Error,
                    format!("CLIENT_NOT_FOUND: get_visible_lines client_id={}", req.client_id),
                );
            });
            Status::not_found(format!("Client {} not found", req.client_id))
        })?;

        // viewport is domain-owned (#753 E3). Use compositor to determine window geometry.
        // Return terminal_size as viewport dimensions (best effort until E5/E6).
        let (tw, th) = state.terminal_size;

        let window_id = requested_window_id.unwrap_or_else(|| {
            // Use focused window from compositor, or window_id 0 as fallback
            state.compositor.as_ref().map_or(0, |c| {
                let screen = reovim_subsys_layout::Rect::new(0, 0, tw, th);
                c.composite(screen)
                    .focused
                    .map_or(0, |id| id.as_usize() as u64)
            })
        });

        Ok(Response::new(GetVisibleLinesResponse {
            window_id,
            first_line: 0,
            last_line: u64::from(th).saturating_sub(1),
            viewport_height: u64::from(th),
        }))
    }

    /// Get screen content via TUI capture relay.
    ///
    /// This implements the CLI→Server→TUI→Server→CLI capture flow:
    /// 1. Creates a pending capture request with unique ID
    /// 2. Sends `capture_request` notification to connected TUI client
    /// 3. Waits for TUI to respond with `capture_response` notification
    /// 4. Returns the captured frame content to the CLI
    ///
    /// Requires a connected TUI client. Returns an error if no TUI is connected
    /// or if the capture times out.
    async fn get_screen_content(
        &self,
        request: Request<GetScreenContentRequest>,
    ) -> Result<Response<GetScreenContentResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // Validate format
        let format = if req.format.is_empty() {
            "raw_ansi".to_string() // Default to ANSI
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

        // Get target client ID
        let target_client_id = req.client_id;

        // Create pending capture request
        let (request_id, rx) = session.capture_tracker().create_pending();

        // Build and emit capture_request notification
        let notification = Notification {
            event_type: "capture_request".to_string(),
            // Phase #479: System time before UNIX_EPOCH is a serious system error
            #[allow(clippy::cast_possible_truncation)] // Timestamp won't overflow u64
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
        tracing::debug!(request_id, target_client_id, format, "Sent capture_request notification");

        // Wait for TUI response with timeout
        let result = wait_for_capture(rx).await.map_err(|e| {
            // Cancel the pending request on error
            session.capture_tracker().cancel(request_id);
            capture_error_to_status(e)
        })?;

        tracing::debug!(
            request_id,
            width = result.width,
            height = result.height,
            "Received capture response"
        );

        Ok(Response::new(GetScreenContentResponse {
            width: result.width,
            height: result.height,
            format: result.format,
            content: result.content,
        }))
    }

    /// Submit captured screen content (TUI → Server part of capture relay).
    ///
    /// Called by TUI in response to a `capture_request` notification.
    /// Delivers the captured frame to the pending `GetScreenContent` request.
    async fn submit_capture_response(
        &self,
        request: Request<SubmitCaptureRequest>,
    ) -> Result<Response<SubmitCaptureResponseReply>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        let result = CaptureResult {
            width: req.width,
            height: req.height,
            format: req.format,
            content: req.content,
        };

        let ok = session
            .capture_tracker()
            .deliver_response(req.request_id, result);
        tracing::debug!(request_id = req.request_id, ok, "Delivered capture response");

        Ok(Response::new(SubmitCaptureResponseReply { ok }))
    }

    /// Get register contents from per-client state (#515).
    ///
    /// If names are specified, returns only those registers.
    /// If names is empty, returns all non-empty registers.
    async fn get_registers(
        &self,
        request: Request<GetRegistersRequest>,
    ) -> Result<Response<GetRegistersResponse>, Status> {
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_id = resolve_target_client_id(token_client_id, req.client_id)?;

        // Verify client exists
        let _state = session.clients().client_state(client_id).ok_or_else(|| {
            Status::not_found(format!("Client {} not found", client_id.as_usize()))
        })?;

        // registers are domain-owned (#753 E3) — return empty until E5/E6 wires domain query.
        // The domain driver will provide register contents via projections.
        let registers: Vec<RegisterEntry> = Vec::new();

        Ok(Response::new(GetRegistersResponse { registers }))
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
