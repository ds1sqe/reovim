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
        SubmitCaptureRequest, SubmitCaptureResponseReply, notification::Payload,
        state_service_server::StateService,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{CaptureResult, Session, SessionId, SessionRegistry, wait_for_capture};

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
    /// Returns the current mode from the `driver_session`'s mode stack.
    async fn get_mode(
        &self,
        _request: Request<GetModeRequest>,
    ) -> Result<Response<GetModeResponse>, Status> {
        let session = self.get_session()?;

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
    #[allow(clippy::cast_possible_truncation)]
    async fn get_cursor(
        &self,
        _request: Request<GetCursorRequest>,
    ) -> Result<Response<GetCursorResponse>, Status> {
        let session = self.get_session()?;

        // Get cursor from active buffer
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
    /// Not yet implemented - requires layout compositor integration.
    async fn get_layout(
        &self,
        _request: Request<GetLayoutRequest>,
    ) -> Result<Response<GetLayoutResponse>, Status> {
        Err(Status::unimplemented("GetLayout not yet implemented"))
    }

    /// Get visible line range for a window.
    ///
    /// Not yet implemented - requires viewport tracking.
    async fn get_visible_lines(
        &self,
        _request: Request<GetVisibleLinesRequest>,
    ) -> Result<Response<GetVisibleLinesResponse>, Status> {
        Err(Status::unimplemented("GetVisibleLines not yet implemented"))
    }

    /// Get selection state.
    ///
    /// Not yet implemented - requires visual mode integration.
    async fn get_selection(
        &self,
        _request: Request<GetSelectionRequest>,
    ) -> Result<Response<GetSelectionResponse>, Status> {
        Err(Status::unimplemented("GetSelection not yet implemented"))
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

        let request = Request::new(GetModeRequest {});
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

        let request = Request::new(GetCursorRequest { window_id: None });
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

        let request = Request::new(GetCursorRequest { window_id: None });
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
    async fn test_get_layout_unimplemented() {
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetLayoutRequest {});
        let response = service.get_layout(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_get_mode_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = StateServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(GetModeRequest {});
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
}
