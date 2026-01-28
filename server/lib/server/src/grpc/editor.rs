//! `EditorService` gRPC implementation.
//!
//! Provides editor-level operations for v2 protocol clients.
//!
//! Note: `Resize` is not implemented per architecture design - clients handle
//! their own layout. See `docs/architecture/server-client-split.md`.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{
        GetActiveBufferRequest, GetActiveBufferResponse, QuitRequest, QuitResponse, ResizeRequest,
        ResizeResponse, SetActiveBufferRequest, SetActiveBufferResponse,
        editor_service_server::EditorService,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{Session, SessionId, SessionRegistry};

/// gRPC `EditorService` implementation.
///
/// Bridges v2 protocol editor operations to the session system.
/// Note: `Resize` returns unimplemented - clients handle their own layout.
pub struct EditorServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
}

impl EditorServiceImpl {
    /// Create a new `EditorService` with access to the session registry.
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
impl EditorService for EditorServiceImpl {
    /// Resize the viewport.
    ///
    /// **Not implemented**: Per the server/client architecture, clients handle
    /// their own layout and terminal dimensions. The server provides raw data,
    /// clients handle presentation.
    ///
    /// See `docs/architecture/server-client-split.md` for details.
    async fn resize(
        &self,
        _request: Request<ResizeRequest>,
    ) -> Result<Response<ResizeResponse>, Status> {
        // Per architecture: "EditorService.Resize - Client handles its own layout"
        // Clients manage their own terminal/window dimensions.
        Err(Status::unimplemented(
            "Resize not implemented - clients handle their own layout",
        ))
    }

    /// Quit the editor.
    ///
    /// Note: Full implementation requires shutdown signaling from the Server
    /// struct. Currently returns unimplemented.
    async fn quit(&self, _request: Request<QuitRequest>) -> Result<Response<QuitResponse>, Status> {
        // Quit requires server-level shutdown signaling which isn't wired
        // through lib/server yet. The runner's server has this capability.
        Err(Status::unimplemented("Quit not yet implemented - use ServerService.Kill"))
    }

    /// Set the active buffer.
    #[allow(clippy::cast_possible_truncation)]
    async fn set_active_buffer(
        &self,
        request: Request<SetActiveBufferRequest>,
    ) -> Result<Response<SetActiveBufferResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        let buffer_id = BufferId::from_raw(req.buffer_id as usize);

        // Verify buffer exists and set as active
        let ok = session
            .with_state_mut(|state| {
                if state.buffer(buffer_id).is_some() {
                    state.set_active_buffer(Some(buffer_id));
                    true
                } else {
                    false
                }
            })
            .await;

        if ok {
            Ok(Response::new(SetActiveBufferResponse { ok: true }))
        } else {
            Err(Status::not_found(format!("Buffer {} not found", req.buffer_id)))
        }
    }

    /// Get the active buffer ID.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_active_buffer(
        &self,
        _request: Request<GetActiveBufferRequest>,
    ) -> Result<Response<GetActiveBufferResponse>, Status> {
        let session = self.get_session()?;

        let buffer_id = session
            .with_state(|state| state.active_buffer().map(|id| id.as_usize() as u64))
            .await;

        Ok(Response::new(GetActiveBufferResponse { buffer_id }))
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
    async fn test_resize_unimplemented() {
        let registry = test_registry();
        let service = EditorServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(ResizeRequest {
            width: 80,
            height: 24,
        });
        let response = service.resize(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_quit_unimplemented() {
        let registry = test_registry();
        let service = EditorServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(QuitRequest { force: false });
        let response = service.quit(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_get_active_buffer_none() {
        let registry = test_registry();
        let service = EditorServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetActiveBufferRequest {});
        let response = service.get_active_buffer(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.buffer_id.is_none());
    }

    #[tokio::test]
    async fn test_get_active_buffer_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer
        session
            .with_state_mut(|state| {
                state.create_buffer("test content");
            })
            .await;

        let service = EditorServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetActiveBufferRequest {});
        let response = service.get_active_buffer(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.buffer_id.is_some());
    }

    #[tokio::test]
    async fn test_set_active_buffer_not_found() {
        let registry = test_registry();
        let service = EditorServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SetActiveBufferRequest { buffer_id: 999 });
        let response = service.set_active_buffer(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_set_active_buffer_success() {
        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer
        let buffer_id = session
            .with_state_mut(|state| {
                let id = state.create_buffer("test content");
                id.as_usize() as u64
            })
            .await;

        let service = EditorServiceImpl::new(registry.clone(), SessionId::new("test"));

        let request = Request::new(SetActiveBufferRequest { buffer_id });
        let response = service.set_active_buffer(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }
}
