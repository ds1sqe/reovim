//! `StateService` gRPC implementation.
//!
//! Provides editor state queries for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_protocol::v2::{
        CaptureRequestPayload, GetCursorRequest, GetCursorResponse, GetLayoutRequest,
        GetLayoutResponse, GetModeRequest, GetModeResponse, GetOptionsRequest, GetOptionsResponse,
        GetRegistersRequest, GetRegistersResponse, GetScreenContentRequest,
        GetScreenContentResponse, GetSelectionRequest, GetSelectionResponse,
        GetVisibleLinesRequest, GetVisibleLinesResponse, Notification, Position, RegisterEntry,
        SplitDirection, SubmitCaptureRequest, SubmitCaptureResponseReply, WindowLeaf, WindowNode,
        WindowRect, WindowSplit, notification::Payload, state_service_server::StateService,
        window_node::Node,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{
    CaptureResult, ClientId, Session, SessionId, SessionRegistry, wait_for_capture,
};

/// Convert a driver-layer `Window` to a proto `WindowLeaf`.
///
/// This conversion happens in the gRPC layer to keep the driver crate
/// free of protocol dependencies.
#[allow(clippy::cast_possible_truncation)]
fn window_to_leaf(window: &reovim_driver_session::Window) -> WindowLeaf {
    WindowLeaf {
        window_id: window.id.as_usize() as u64,
        buffer_id: window.buffer_id.map_or(0, |id| id.as_usize() as u64),
        rect: Some(WindowRect {
            x: 0, // Position calculated by client based on layout
            y: 0,
            width: u64::from(window.viewport.width),
            height: u64::from(window.viewport.height),
        }),
    }
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
    /// Get the current editor mode.
    ///
    /// Returns the current mode from the `driver_session`'s mode stack, or
    /// from a specific client's mode stack if `client_id` is provided.
    ///
    /// # Per-client state (#471): Per-client mode isolation
    ///
    /// When `client_id > 0`, returns the mode from that client's per-client
    /// mode stack (stored in `Client::Owner { state: EditingState }`).
    /// When `client_id == 0` (default), returns the shared session mode
    /// for backward compatibility.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_mode(
        &self,
        request: Request<GetModeRequest>,
    ) -> Result<Response<GetModeResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // Per-client state (#471): Check for per-client mode first
        if req.client_id > 0 {
            let client_id = ClientId::new(req.client_id as usize);

            // Try to get per-client mode
            if let Some(mode) = session.client_current_mode(client_id) {
                let name = mode.name().to_string();
                let display = name.to_uppercase();
                let is_insert = name.contains("insert") || name.contains("cmdline");

                return Ok(Response::new(GetModeResponse {
                    name,
                    display,
                    is_insert,
                }));
            }
            // Client not found - fall through to shared mode
            tracing::debug!(%client_id, "Client not found, returning shared mode");
        }

        // Fallback: Return shared session mode (backward compatibility)
        // Per-client state (#471): This is a valid fallback for clients without client_id.
        #[allow(deprecated)]
        let (name, display, is_insert) = session
            .with_state(|state| {
                let mode = state.current_mode();
                let name = mode.name().to_string();
                // Display name: uppercase the mode name
                let display = name.to_uppercase();
                // Check if this is an insert-like mode
                let is_insert = name.contains("insert") || name.contains("cmdline");
                (name, display, is_insert)
            })
            .await;

        Ok(Response::new(GetModeResponse {
            name,
            display,
            is_insert,
        }))
    }

    /// Get cursor position in the active window/buffer.
    ///
    /// # Phase #471: Per-client cursor isolation
    ///
    /// When `client_id > 0`, returns the cursor from that client's per-client
    /// window layout (stored in `EditingState.windows`).
    /// When `client_id == 0` (default), falls back to shared session state
    /// for backward compatibility.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_cursor(
        &self,
        request: Request<GetCursorRequest>,
    ) -> Result<Response<GetCursorResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // Phase #471: Try per-client cursor first when client_id > 0
        if req.client_id > 0 {
            let client_id = ClientId::new(req.client_id as usize);

            // Get cursor from per-client window layout
            if let Some(state) = session.client_state(client_id)
                && let Some(window) = state.windows.active()
            {
                let cursor = &window.cursor;
                return Ok(Response::new(GetCursorResponse {
                    window_id: window.id.as_usize() as u64,
                    position: Some(Position {
                        line: cursor.line as u64,
                        column: cursor.column as u64,
                    }),
                }));
            }
            // Client not found - fall through to shared state
            tracing::debug!(%client_id, "Client not found, returning shared cursor");
        }

        // Fallback: Get cursor from shared buffer state (backward compatibility)
        let (window_id, position) = session
            .with_state(|state| {
                // Collapse nested if-let and merge read() with position() to avoid drop timing lint
                if let Some(buffer_id) = state.active_buffer()
                    && let Some(buffer_arc) = state.buffer(buffer_id)
                {
                    let pos = buffer_arc.read().position();
                    return Some((
                        // No window management in lib/server yet, use buffer_id as window_id
                        buffer_id.as_usize() as u64,
                        Position {
                            line: pos.line as u64,
                            column: pos.column as u64,
                        },
                    ));
                }
                None
            })
            .await
            .ok_or_else(|| Status::not_found("No active buffer"))?;

        Ok(Response::new(GetCursorResponse {
            window_id,
            position: Some(position),
        }))
    }

    /// Get editor options.
    ///
    /// Not yet implemented - requires option registry integration.
    async fn get_options(
        &self,
        _request: Request<GetOptionsRequest>,
    ) -> Result<Response<GetOptionsResponse>, Status> {
        Err(Status::unimplemented("GetOptions not yet implemented"))
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
    /// When `client_id > 0`, returns the layout from that client's per-client
    /// window layout (stored in `EditingState.windows`).
    /// When `client_id == 0` (default), falls back to shared session layout
    /// for backward compatibility.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_layout(
        &self,
        request: Request<GetLayoutRequest>,
    ) -> Result<Response<GetLayoutResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // Phase #471: Try per-client layout first when client_id > 0
        if req.client_id > 0 {
            let client_id = ClientId::new(req.client_id as usize);

            // Get layout from per-client editing state
            if let Some(state) = session.client_state(client_id) {
                let layout = &state.windows;
                let focused_id = layout.active_id().map_or(0, |id| id.as_usize() as u64);

                let root = match layout.len() {
                    0 => None,
                    1 => layout.active().map(|w| WindowNode {
                        node: Some(Node::Leaf(window_to_leaf(w))),
                    }),
                    _ => {
                        let children: Vec<WindowNode> = layout
                            .windows
                            .iter()
                            .map(|w| WindowNode {
                                node: Some(Node::Leaf(window_to_leaf(w))),
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

                return Ok(Response::new(GetLayoutResponse {
                    root,
                    focused_window_id: focused_id,
                }));
            }
            // Client not found - fall through to shared state
            tracing::debug!(%client_id, "Client not found, returning shared layout");
        }

        // Fallback: Get layout from shared session state (backward compatibility)
        let (root, focused_window_id) = session
            .with_state(|state| {
                let driver_session = state.driver_session();
                let layout = &driver_session.windows;

                // Get focused window ID
                let focused_id = layout.active_id().map_or(0, |id| id.as_usize() as u64);

                // Build window tree
                let root = match layout.len() {
                    0 => None,
                    1 => {
                        // Single window: return as leaf
                        layout.active().map(|w| WindowNode {
                            node: Some(Node::Leaf(window_to_leaf(w))),
                        })
                    }
                    _ => {
                        // Multiple windows: wrap in vertical split
                        let children: Vec<WindowNode> = layout
                            .windows
                            .iter()
                            .map(|w| WindowNode {
                                node: Some(Node::Leaf(window_to_leaf(w))),
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
            })
            .await;

        Ok(Response::new(GetLayoutResponse {
            root,
            focused_window_id,
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
    /// - `client_id`: Optional client ID for per-client viewport (Phase #471)
    ///
    /// # Returns
    ///
    /// - `first_line`: First visible line (0-indexed, based on `scroll_top`)
    /// - `last_line`: Last visible line (0-indexed, `scroll_top` + height - 1)
    /// - `viewport_height`: Number of visible lines
    ///
    /// # Phase #471: Per-client viewport isolation
    ///
    /// When `client_id > 0`, returns visible lines from that client's per-client
    /// window layout. When `client_id == 0` (default), falls back to shared
    /// session layout for backward compatibility.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_visible_lines(
        &self,
        request: Request<GetVisibleLinesRequest>,
    ) -> Result<Response<GetVisibleLinesResponse>, Status> {
        let req = request.into_inner();
        let requested_window_id = req.window_id;
        let session = self.get_session()?;

        // Phase #471: Try per-client viewport first when client_id > 0
        if req.client_id > 0 {
            let client_id = ClientId::new(req.client_id as usize);

            if let Some(state) = session.client_state(client_id) {
                let layout = &state.windows;

                let window = requested_window_id.map_or_else(
                    || layout.active(),
                    |id| layout.windows.iter().find(|w| w.id.as_usize() as u64 == id),
                );

                if let Some(w) = window {
                    let viewport = &w.viewport;
                    return Ok(Response::new(GetVisibleLinesResponse {
                        window_id: w.id.as_usize() as u64,
                        first_line: viewport.scroll_top as u64,
                        last_line: viewport.last_visible_line() as u64,
                        viewport_height: u64::from(viewport.height),
                    }));
                }
            }
            // Client not found - fall through to shared state
            tracing::debug!(%client_id, "Client not found, returning shared visible lines");
        }

        // Fallback: Get visible lines from shared session state (backward compatibility)
        let result = session
            .with_state(|state| {
                let driver_session = state.driver_session();
                let layout = &driver_session.windows;

                // Find the requested window or use active
                let window = requested_window_id.map_or_else(
                    || layout.active(),
                    |id| layout.windows.iter().find(|w| w.id.as_usize() as u64 == id),
                );

                window.map(|w| {
                    let viewport = &w.viewport;
                    (
                        w.id.as_usize() as u64,
                        viewport.scroll_top as u64,
                        viewport.last_visible_line() as u64,
                        u64::from(viewport.height),
                    )
                })
            })
            .await;

        let (window_id, first_line, last_line, viewport_height) =
            result.ok_or_else(|| Status::not_found("Window not found"))?;

        Ok(Response::new(GetVisibleLinesResponse {
            window_id,
            first_line,
            last_line,
            viewport_height,
        }))
    }

    /// Get selection state.
    ///
    /// Returns the current visual selection bounds and mode if active.
    /// Selection is active when in visual mode (character, line, or block).
    ///
    /// # Phase #471: Per-client selection isolation
    ///
    /// When `client_id > 0`, returns the selection from that client's per-client
    /// window layout. When `client_id == 0` (default), falls back to shared
    /// session layout for backward compatibility.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_selection(
        &self,
        request: Request<GetSelectionRequest>,
    ) -> Result<Response<GetSelectionResponse>, Status> {
        use reovim_protocol::v2::Selection;

        let req = request.into_inner();
        let session = self.get_session()?;

        // Phase #471: Try per-client selection first when client_id > 0
        if req.client_id > 0 {
            let client_id = ClientId::new(req.client_id as usize);

            if let Some(state) = session.client_state(client_id) {
                // Get selection from per-client windows
                let layout = &state.windows;

                // Get active window from per-client layout
                if let Some(window) = layout.active() {
                    if let Some(ref sel) = window.selection {
                        use reovim_driver_session::SelectionMode;

                        let mode_str = match sel.mode {
                            SelectionMode::Character => "char",
                            SelectionMode::Line => "line",
                            SelectionMode::Block => "block",
                        };

                        return Ok(Response::new(GetSelectionResponse {
                            has_selection: true,
                            selection: Some(Selection {
                                start: Some(Position {
                                    line: sel.start.line as u64,
                                    column: sel.start.column as u64,
                                }),
                                end: Some(Position {
                                    line: sel.end.line as u64,
                                    column: sel.end.column as u64,
                                }),
                            }),
                            visual_mode: Some(mode_str.to_string()),
                        }));
                    }
                    // Window exists but no selection
                    return Ok(Response::new(GetSelectionResponse {
                        has_selection: false,
                        selection: None,
                        visual_mode: None,
                    }));
                }
            }
            // Client not found - fall through to shared state
            tracing::debug!(%client_id, "Client not found, returning shared selection");
        }

        // Fallback: Get selection from shared session state (backward compatibility)
        let (has_selection, selection, visual_mode) = session
            .with_state(|state| {
                // Phase 8 (#465): Read selection from WINDOW, not buffer.
                let driver_session = state.driver_session();

                // Get buffer ID from active buffer
                let Some(buffer_id) = state.active_buffer() else {
                    return (false, None, None);
                };

                // Find focused window first, fallback to any window displaying buffer
                let window = {
                    let focused = driver_session.windows.active();
                    if focused.is_some_and(|f| f.buffer_id == Some(buffer_id)) {
                        focused
                    } else {
                        driver_session
                            .windows
                            .windows
                            .iter()
                            .find(|w| w.buffer_id == Some(buffer_id))
                    }
                };

                let Some(window) = window else {
                    return (false, None, None);
                };

                // Phase 8 (#465): Selection now lives in Window with explicit start/end.
                // Read directly from window.selection.
                if let Some(ref sel) = window.selection {
                    use reovim_driver_session::SelectionMode;

                    // Map SelectionMode to protocol string
                    let mode_str = match sel.mode {
                        SelectionMode::Character => "char",
                        SelectionMode::Line => "line",
                        SelectionMode::Block => "block",
                    };

                    return (
                        true,
                        Some(Selection {
                            start: Some(Position {
                                line: sel.start.line as u64,
                                column: sel.start.column as u64,
                            }),
                            end: Some(Position {
                                line: sel.end.line as u64,
                                column: sel.end.column as u64,
                            }),
                        }),
                        Some(mode_str.to_string()),
                    );
                }

                // No active selection
                (false, None, None)
            })
            .await;

        Ok(Response::new(GetSelectionResponse {
            has_selection,
            selection,
            visual_mode,
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

        // Create pending capture request
        let (request_id, rx) = session.capture_tracker().create_pending();

        // Build and emit capture_request notification
        let notification = Notification {
            event_type: "capture_request".to_string(),
            #[allow(clippy::cast_possible_truncation)] // Timestamp won't overflow u64
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
            payload: Some(Payload::CaptureRequest(CaptureRequestPayload {
                request_id,
                format: format.clone(),
            })),
        };

        session.emit_notification(notification);
        tracing::debug!(request_id, format, "Sent capture_request notification");

        // Wait for TUI response with timeout
        let result = wait_for_capture(rx).await.map_err(|e| {
            // Cancel the pending request on error
            session.capture_tracker().cancel(request_id);
            match e {
                crate::session::CaptureError::NoTuiClient => Status::unavailable(e.to_string()),
                crate::session::CaptureError::Timeout => Status::deadline_exceeded(e.to_string()),
                crate::session::CaptureError::Disconnected => Status::aborted(e.to_string()),
                crate::session::CaptureError::InvalidResponse(msg) => Status::internal(msg),
            }
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

    /// Get register contents.
    ///
    /// If names are specified, returns only those registers.
    /// If names is empty, returns all non-empty registers.
    async fn get_registers(
        &self,
        request: Request<GetRegistersRequest>,
    ) -> Result<Response<GetRegistersResponse>, Status> {
        use reovim_kernel::api::v1::YankType;

        let req = request.into_inner();
        let session = self.get_session()?;

        let registers = session
            .with_state(|state| {
                let bank = state.app.kernel.registers.read();

                if req.names.is_empty() {
                    // Return all non-empty registers
                    bank.iter_non_empty()
                        .map(|(name, content)| {
                            let yank_type_str = match content.yank_type {
                                YankType::Characterwise => "char",
                                YankType::Linewise => "line",
                            };
                            RegisterEntry {
                                name: name.to_string(),
                                content_type: "text".to_string(),
                                content: content.text.clone(),
                                yank_type: yank_type_str.to_string(),
                            }
                        })
                        .collect::<Vec<_>>()
                } else {
                    // Return only requested registers
                    req.names
                        .iter()
                        .filter_map(|name| {
                            let name_char = name.chars().next()?;
                            let content = bank.get_by_name(Some(name_char))?;
                            if content.is_empty() {
                                return None;
                            }
                            let yank_type_str = match content.yank_type {
                                YankType::Characterwise => "char",
                                YankType::Linewise => "line",
                            };
                            Some(RegisterEntry {
                                name: name_char.to_string(),
                                content_type: "text".to_string(),
                                content: content.text.clone(),
                                yank_type: yank_type_str.to_string(),
                            })
                        })
                        .collect()
                }
            })
            .await;

        Ok(Response::new(GetRegistersResponse { registers }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> Arc<SessionRegistry> {
        let registry = Arc::new(SessionRegistry::new());
        let session = Arc::new(Session::new(SessionId::new("test")));
        registry.insert(&session);
        registry
    }

    /// Create a registry with a session that has a real buffer manager.
    fn test_registry_with_buffer_manager() -> (Arc<SessionRegistry>, Arc<Session>) {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
        };

        // Create a KernelContext with a real buffer manager
        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = crate::session::SessionState::with_kernel(kernel);
        let session = Arc::new(Session::from_state(SessionId::new("test"), state));

        let registry = Arc::new(SessionRegistry::new());
        registry.insert(&session);

        (registry, session)
    }

    #[tokio::test]
    async fn test_get_mode_returns_current_mode() {
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetModeRequest { client_id: 0 });
        let response = service.get_mode(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        // Default mode is "normal" from SessionState::default()
        assert_eq!(resp.name, "normal");
        assert_eq!(resp.display, "NORMAL");
        assert!(!resp.is_insert);
    }

    #[tokio::test]
    async fn test_get_cursor_no_buffer() {
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetCursorRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_cursor(request).await;

        // No buffer = NotFound
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_cursor_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer using the real buffer manager
        session
            .with_state_mut(|state| {
                state.create_buffer("hello world");
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetCursorRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_cursor(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.position.is_some());
        let pos = resp.position.unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
    }

    #[tokio::test]
    async fn test_get_options_unimplemented() {
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetOptionsRequest { names: vec![] });
        let response = service.get_options(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_get_layout_empty() {
        // Session with no windows
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetLayoutRequest { client_id: 0 });
        let response = service.get_layout(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        // Empty layout: no root node
        assert!(resp.root.is_none());
        assert_eq!(resp.focused_window_id, 0);
    }

    #[tokio::test]
    async fn test_get_layout_single_window() {
        use reovim_driver_session::Viewport;

        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer (which also creates a window via Phase 8 change)
        // Then modify the existing window's viewport
        session
            .with_state_mut(|state| {
                let _buffer_id = state.create_buffer("hello world");
                // Modify the existing window created by create_buffer
                if let Some(window) = state.driver_session_mut().windows.active_mut() {
                    window.viewport = Viewport::new(80, 24);
                }
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetLayoutRequest { client_id: 0 });
        let response = service.get_layout(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();

        // Should have a root node (single leaf)
        assert!(resp.root.is_some());
        let root = resp.root.unwrap();

        // Check it's a leaf
        match root.node {
            Some(reovim_protocol::v2::window_node::Node::Leaf(leaf)) => {
                assert_eq!(leaf.rect.as_ref().unwrap().width, 80);
                assert_eq!(leaf.rect.as_ref().unwrap().height, 24);
            }
            _ => panic!("Expected a leaf node"),
        }
    }

    #[tokio::test]
    async fn test_get_visible_lines_no_window() {
        // Session with no windows
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetVisibleLinesRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_visible_lines(request).await;

        // No window = NotFound
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_visible_lines_with_window() {
        use reovim_driver_session::Viewport;

        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer (which also creates a window via Phase 8 change)
        // Then modify the existing window's viewport
        session
            .with_state_mut(|state| {
                let _buffer_id = state.create_buffer("line0\nline1\nline2\nline3");
                if let Some(window) = state.driver_session_mut().windows.active_mut() {
                    window.viewport = Viewport::new(80, 24);
                    // scroll_top is 0 by default
                }
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetVisibleLinesRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_visible_lines(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();

        assert_eq!(resp.first_line, 0);
        assert_eq!(resp.last_line, 23); // scroll_top(0) + height(24) - 1
        assert_eq!(resp.viewport_height, 24);
    }

    #[tokio::test]
    async fn test_get_visible_lines_with_scroll() {
        use reovim_driver_session::Viewport;

        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer (which also creates a window via Phase 8 change)
        // Then modify the existing window's viewport with scroll
        session
            .with_state_mut(|state| {
                let _buffer_id = state.create_buffer("content");
                if let Some(window) = state.driver_session_mut().windows.active_mut() {
                    let mut viewport = Viewport::new(80, 24);
                    viewport.scroll_top = 10; // Scrolled down 10 lines
                    window.viewport = viewport;
                }
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetVisibleLinesRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_visible_lines(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();

        assert_eq!(resp.first_line, 10);
        assert_eq!(resp.last_line, 33); // scroll_top(10) + height(24) - 1
        assert_eq!(resp.viewport_height, 24);
    }

    #[tokio::test]
    async fn test_get_mode_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = StateServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(GetModeRequest { client_id: 0 });
        let response = service.get_mode(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_registers_empty() {
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetRegistersRequest { names: vec![] });
        let response = service.get_registers(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        // Empty by default
        assert!(resp.registers.is_empty());
    }

    #[tokio::test]
    async fn test_get_registers_with_content() {
        use reovim_kernel::api::v1::RegisterContent;

        let (registry, session) = test_registry_with_buffer_manager();

        // Set a register
        session
            .with_state_mut(|state| {
                state
                    .app
                    .kernel
                    .registers
                    .write()
                    .set(RegisterContent::characterwise("hello"));
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetRegistersRequest { names: vec![] });
        let response = service.get_registers(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.registers.len(), 1);
        assert_eq!(resp.registers[0].name, "\"");
        assert_eq!(resp.registers[0].content, "hello");
        assert_eq!(resp.registers[0].yank_type, "char");
    }

    #[tokio::test]
    async fn test_get_registers_specific_register() {
        use reovim_kernel::api::v1::RegisterContent;

        let (registry, session) = test_registry_with_buffer_manager();

        // Set multiple registers
        session
            .with_state_mut(|state| {
                let mut bank = state.app.kernel.registers.write();
                bank.set(RegisterContent::characterwise("unnamed"));
                bank.set_named('a', RegisterContent::linewise("alpha"));
                bank.set_named('b', RegisterContent::characterwise("beta"));
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        // Query only register 'a'
        let request = Request::new(GetRegistersRequest {
            names: vec!["a".to_string()],
        });
        let response = service.get_registers(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.registers.len(), 1);
        assert_eq!(resp.registers[0].name, "a");
        assert_eq!(resp.registers[0].content, "alpha");
        assert_eq!(resp.registers[0].yank_type, "line");
    }

    // Phase 9.1: GetSelection RPC tests

    #[tokio::test]
    async fn test_get_selection_no_selection() {
        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer but don't start selection
        session
            .with_state_mut(|state| {
                state.create_buffer("hello world");
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_selection(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.has_selection);
        assert!(resp.selection.is_none());
        assert!(resp.visual_mode.is_none());
    }

    #[tokio::test]
    async fn test_get_selection_no_buffer() {
        // Session with no buffer
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_selection(request).await;

        // Should return no selection (graceful handling), not an error
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.has_selection);
    }

    #[tokio::test]
    async fn test_get_selection_with_char_selection() {
        use {
            reovim_driver_session::{Viewport, api::Selection},
            reovim_kernel::api::v1::Position as KernelPosition,
        };

        let (registry, session) = test_registry_with_buffer_manager();

        // Create buffer and set up window with selection
        // Phase 8 (#465): Selection now lives in Window, not Buffer.
        // create_buffer() already creates a Window, so modify it rather than adding a new one.
        session
            .with_state_mut(|state| {
                let _buffer_id = state.create_buffer("hello world");
                // Modify the existing window created by create_buffer
                if let Some(window) = state.driver_session_mut().windows.active_mut() {
                    window.viewport = Viewport::new(80, 24);
                    // Set selection for "hello" (0,0 to 0,5 exclusive)
                    window.selection = Some(Selection::character(
                        KernelPosition::new(0, 0),
                        KernelPosition::new(0, 5),
                    ));
                }
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_selection(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.has_selection);
        assert!(resp.selection.is_some());

        let sel = resp.selection.unwrap();
        assert_eq!(sel.start.as_ref().unwrap().line, 0);
        assert_eq!(sel.start.as_ref().unwrap().column, 0);
        assert_eq!(sel.end.as_ref().unwrap().line, 0);
        assert_eq!(sel.end.as_ref().unwrap().column, 5); // exclusive end
        assert_eq!(resp.visual_mode, Some("char".to_string()));
    }

    #[tokio::test]
    async fn test_get_selection_line_mode() {
        use {
            reovim_driver_session::{Viewport, api::Selection},
            reovim_kernel::api::v1::Position as KernelPosition,
        };

        let (registry, session) = test_registry_with_buffer_manager();

        // Phase 8 (#465): Selection now lives in Window.
        // create_buffer() already creates a Window, so modify it rather than adding a new one.
        session
            .with_state_mut(|state| {
                let _buffer_id = state.create_buffer("line1\nline2\nline3");
                if let Some(window) = state.driver_session_mut().windows.active_mut() {
                    window.viewport = Viewport::new(80, 24);
                    // Line-wise selection for lines 0-1
                    window.selection = Some(Selection::line(
                        KernelPosition::new(0, 0),
                        KernelPosition::new(2, 0), // exclusive end
                    ));
                }
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_selection(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.has_selection);
        assert_eq!(resp.visual_mode, Some("line".to_string()));
    }

    #[tokio::test]
    async fn test_get_selection_block_mode() {
        use {
            reovim_driver_session::{Viewport, api::Selection},
            reovim_kernel::api::v1::Position as KernelPosition,
        };

        let (registry, session) = test_registry_with_buffer_manager();

        // Phase 8 (#465): Selection now lives in Window.
        // create_buffer() already creates a Window, so modify it rather than adding a new one.
        session
            .with_state_mut(|state| {
                let _buffer_id = state.create_buffer("ABC\nDEF\nGHI");
                if let Some(window) = state.driver_session_mut().windows.active_mut() {
                    window.viewport = Viewport::new(80, 24);
                    // Block selection from (0,0) to (1,2) - a 2x2 block
                    window.selection = Some(Selection::block(
                        KernelPosition::new(0, 0),
                        KernelPosition::new(2, 2), // exclusive end
                    ));
                }
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_selection(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.has_selection);
        assert_eq!(resp.visual_mode, Some("block".to_string()));
    }

    #[tokio::test]
    async fn test_get_selection_reverse() {
        use {
            reovim_driver_session::{Viewport, api::Selection},
            reovim_kernel::api::v1::Position as KernelPosition,
        };

        let (registry, session) = test_registry_with_buffer_manager();

        // Phase 8 (#465): Selection now lives in Window.
        // Selection stores start/end directly - normalized to start < end.
        // create_buffer() already creates a Window, so modify it rather than adding a new one.
        session
            .with_state_mut(|state| {
                let _buffer_id = state.create_buffer("hello world");
                if let Some(window) = state.driver_session_mut().windows.active_mut() {
                    window.viewport = Viewport::new(80, 24);
                    // Selection from column 2 to column 5 (already normalized)
                    window.selection = Some(Selection::character(
                        KernelPosition::new(0, 2),
                        KernelPosition::new(0, 5),
                    ));
                }
            })
            .await;

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 0,
        });
        let response = service.get_selection(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.has_selection);

        let sel = resp.selection.unwrap();
        // start should be normalized (smaller position comes first)
        assert!(sel.start.as_ref().unwrap().column < sel.end.as_ref().unwrap().column);
        assert_eq!(sel.start.as_ref().unwrap().column, 2);
        assert_eq!(sel.end.as_ref().unwrap().column, 5);
    }

    // Per-client state (#471): Per-client mode isolation tests

    #[tokio::test]
    async fn test_get_mode_per_client_returns_client_mode() {
        use reovim_kernel::api::v1::{ModeId, ModuleId};

        let (registry, session) = test_registry_with_buffer_manager();

        // Add a client with custom mode stack
        let client_id = crate::session::ClientId::new(42);
        session.add_client(client_id);

        // Modify the client's per-client mode stack to INSERT mode
        let module = ModuleId::new("editor");
        session.update_client_state(client_id, |editing_state| {
            editing_state
                .mode_stack
                .push(ModeId::new(module.clone(), "insert"));
        });

        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        // Query with client_id = 42 should return INSERT mode
        let request = Request::new(GetModeRequest { client_id: 42 });
        let response = service.get_mode(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.name, "insert");
        assert_eq!(resp.display, "INSERT");
        assert!(resp.is_insert);

        // Query with client_id = 0 should return shared mode (NORMAL)
        let request = Request::new(GetModeRequest { client_id: 0 });
        let response = service.get_mode(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.name, "normal");
        assert_eq!(resp.display, "NORMAL");
        assert!(!resp.is_insert);
    }

    #[tokio::test]
    async fn test_get_mode_unknown_client_falls_back_to_shared() {
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        // Query with non-existent client_id should fall back to shared mode
        let request = Request::new(GetModeRequest { client_id: 999 });
        let response = service.get_mode(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        // Should return shared mode (normal by default)
        assert_eq!(resp.name, "normal");
    }

    // =========================================================================
    // Phase #471: Multi-Client Isolation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_cursor_isolation_between_clients() {
        use reovim_driver_session::CursorPosition;

        let (registry, session) = test_registry_with_buffer_manager();

        // Create buffer first
        session
            .with_state_mut(|state| {
                state.create_buffer("line1\nline2\nline3\nline4\nline5");
            })
            .await;

        // Create two clients
        let client_a = crate::session::ClientId::new(1);
        let client_b = crate::session::ClientId::new(2);
        session.add_client(client_a);
        session.add_client(client_b);

        // Client A moves cursor to (3, 5)
        session.update_client_state(client_a, |state| {
            if let Some(window) = state.windows.active_mut() {
                window.cursor = CursorPosition { line: 3, column: 5 };
            }
        });

        // Client B's cursor should still be at default (0, 0)
        let state_b = session.client_state(client_b).unwrap();
        let cursor_b = state_b.windows.active().unwrap().cursor;
        assert_eq!(cursor_b.line, 0, "Client B cursor line should be 0");
        assert_eq!(cursor_b.column, 0, "Client B cursor column should be 0");

        // Verify via gRPC service
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        // Query Client A's cursor
        let request = Request::new(GetCursorRequest {
            window_id: None,
            client_id: 1,
        });
        let response = service.get_cursor(request).await.unwrap().into_inner();
        let pos = response.position.unwrap();
        assert_eq!(pos.line, 3, "Client A gRPC cursor line");
        assert_eq!(pos.column, 5, "Client A gRPC cursor column");

        // Query Client B's cursor - should be independent
        let request = Request::new(GetCursorRequest {
            window_id: None,
            client_id: 2,
        });
        let response = service.get_cursor(request).await.unwrap().into_inner();
        let pos = response.position.unwrap();
        assert_eq!(pos.line, 0, "Client B gRPC cursor line");
        assert_eq!(pos.column, 0, "Client B gRPC cursor column");
    }

    #[tokio::test]
    async fn test_mode_isolation_between_clients() {
        use reovim_kernel::api::v1::{ModeId, ModuleId};

        let (registry, session) = test_registry_with_buffer_manager();

        // Create two clients
        let client_a = crate::session::ClientId::new(10);
        let client_b = crate::session::ClientId::new(20);
        session.add_client(client_a);
        session.add_client(client_b);

        // Client A enters INSERT mode
        let module = ModuleId::new("editor");
        session.update_client_state(client_a, |state| {
            state.mode_stack.push(ModeId::new(module.clone(), "insert"));
        });

        // Client B should still be in NORMAL mode (not affected by A)
        let mode_b = session.client_current_mode(client_b).unwrap();
        assert_eq!(mode_b.name(), "normal", "Client B should remain in normal mode");

        // Verify via gRPC service
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        // Query Client A's mode - should be INSERT
        let request = Request::new(GetModeRequest { client_id: 10 });
        let response = service.get_mode(request).await.unwrap().into_inner();
        assert_eq!(response.name, "insert", "Client A gRPC mode");
        assert!(response.is_insert, "Client A should be in insert mode");

        // Query Client B's mode - should still be NORMAL
        let request = Request::new(GetModeRequest { client_id: 20 });
        let response = service.get_mode(request).await.unwrap().into_inner();
        assert_eq!(response.name, "normal", "Client B gRPC mode");
        assert!(!response.is_insert, "Client B should NOT be in insert mode");
    }

    #[tokio::test]
    async fn test_layout_isolation_per_client() {
        use reovim_driver_session::Viewport;

        let (registry, session) = test_registry_with_buffer_manager();

        // Create buffer first
        session
            .with_state_mut(|state| {
                state.create_buffer("test content");
            })
            .await;

        // Create a client
        let client_a = crate::session::ClientId::new(100);
        session.add_client(client_a);

        // Modify client's window viewport
        session.update_client_state(client_a, |state| {
            if let Some(window) = state.windows.active_mut() {
                window.viewport = Viewport::new(120, 40);
            }
        });

        // Verify via gRPC with client_id
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetLayoutRequest { client_id: 100 });
        let response = service.get_layout(request).await.unwrap().into_inner();

        // Should get per-client layout with the modified viewport
        assert!(response.root.is_some(), "Should have a root node");
        if let Some(node) = response.root
            && let Some(Node::Leaf(leaf)) = node.node
            && let Some(rect) = leaf.rect
        {
            assert_eq!(rect.width, 120, "Per-client window width");
            assert_eq!(rect.height, 40, "Per-client window height");
        } else {
            panic!("Expected a leaf node with rect");
        }
    }

    #[tokio::test]
    async fn test_selection_isolation_per_client() {
        use {
            reovim_driver_session::{Viewport, api::Selection},
            reovim_kernel::api::v1::Position as KernelPosition,
        };

        let (registry, session) = test_registry_with_buffer_manager();

        // Create buffer first
        session
            .with_state_mut(|state| {
                state.create_buffer("hello world");
            })
            .await;

        // Create two clients
        let client_a = crate::session::ClientId::new(50);
        let client_b = crate::session::ClientId::new(60);
        session.add_client(client_a);
        session.add_client(client_b);

        // Client A has a selection
        session.update_client_state(client_a, |state| {
            if let Some(window) = state.windows.active_mut() {
                window.viewport = Viewport::new(80, 24);
                window.selection = Some(Selection::character(
                    KernelPosition::new(0, 0),
                    KernelPosition::new(0, 5),
                ));
            }
        });

        // Client B has no selection (just viewport)
        session.update_client_state(client_b, |state| {
            if let Some(window) = state.windows.active_mut() {
                window.viewport = Viewport::new(80, 24);
                window.selection = None;
            }
        });

        // Verify via gRPC service
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        // Query Client A's selection - should have selection
        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 50,
        });
        let response = service.get_selection(request).await.unwrap().into_inner();
        assert!(response.has_selection, "Client A should have selection");
        assert_eq!(response.visual_mode, Some("char".to_string()), "Client A selection mode");

        // Query Client B's selection - should have NO selection
        let request = Request::new(GetSelectionRequest {
            window_id: None,
            client_id: 60,
        });
        let response = service.get_selection(request).await.unwrap().into_inner();
        assert!(!response.has_selection, "Client B should NOT have selection");
    }
}
