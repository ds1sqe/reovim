//! `StateService` gRPC implementation.
//!
//! Provides editor state queries for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_protocol::v2::{
        GetCursorRequest, GetCursorResponse, GetLayoutRequest, GetLayoutResponse, GetModeRequest,
        GetModeResponse, GetOptionsRequest, GetOptionsResponse, GetSelectionRequest,
        GetSelectionResponse, GetVisibleLinesRequest, GetVisibleLinesResponse, Position,
        state_service_server::StateService,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{Session, SessionId, SessionRegistry};

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
    /// Note: Mode tracking requires additional infrastructure beyond basic
    /// `lib/server`. Returns a stub "normal" mode for now.
    async fn get_mode(
        &self,
        _request: Request<GetModeRequest>,
    ) -> Result<Response<GetModeResponse>, Status> {
        let _session = self.get_session()?;

        // Mode tracking is managed in the runner's driver_session,
        // which is not available in lib/server. Return stub for now.
        // Full mode tracking will be added as infrastructure grows.
        Ok(Response::new(GetModeResponse {
            name: "normal".to_string(),
            display: "NORMAL".to_string(),
            is_insert: false,
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
        let session = Arc::new(Session::new_with_state(SessionId::new("test"), state));

        let registry = Arc::new(SessionRegistry::new());
        registry.insert(&session);

        (registry, session)
    }

    #[tokio::test]
    async fn test_get_mode_returns_stub() {
        let registry = test_registry();
        let service = StateServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetModeRequest {});
        let response = service.get_mode(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
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
}
