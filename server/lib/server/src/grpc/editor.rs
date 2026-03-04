//! `EditorService` gRPC implementation.
//!
//! Provides editor-level operations for v2 protocol clients.
//!
//! # Viewport Size
//!
//! While clients handle their own layout and rendering (per the server/client
//! architecture), the server needs to know viewport dimensions for:
//! - Scroll commands (`Ctrl-D`, `Ctrl-U`) that move by half-page
//! - Computing visible line range for syntax token optimization
//! - Screen capture in headless TUI mode

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

use crate::session::{ClientId, Session, SessionId, SessionRegistry};

/// gRPC `EditorService` implementation.
///
/// Bridges v2 protocol editor operations to the session system.
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
    /// Relays resize request to connected TUI clients via notification.
    /// The server has no screen - this is purely a relay:
    ///
    /// ```text
    /// CLI (debug) ──► Server (relay) ──► TUI (resizes frame buffer)
    /// ```
    ///
    /// # Arguments
    ///
    /// * `request` - Contains `width` and `height` for the TUI viewport.
    async fn resize(
        &self,
        request: Request<ResizeRequest>,
    ) -> Result<Response<ResizeResponse>, Status> {
        use reovim_protocol::v2::{Notification, ResizeRequestPayload, notification::Payload};

        // Extract caller's client_id from token (if authenticated).
        // Used to target the resize notification to a specific TUI.
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // Relay to TUI via notification
        #[allow(clippy::cast_possible_truncation)]
        let notification = Notification {
            event_type: "resize_request".to_string(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before UNIX_EPOCH")
                .as_millis() as u64,
            payload: Some(Payload::ResizeRequest(ResizeRequestPayload {
                width: req.width,
                height: req.height,
                target_client_id: client_id.map_or(0, |id| id.as_usize() as u64),
            })),
        };

        session.emit_notification(notification);

        tracing::debug!(width = req.width, height = req.height, "Resize relayed to TUI");

        Ok(Response::new(ResizeResponse { ok: true }))
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
    ///
    /// Per-client `active_buffer` (#471): sets the calling client's active buffer.
    #[allow(clippy::cast_possible_truncation)]
    async fn set_active_buffer(
        &self,
        request: Request<SetActiveBufferRequest>,
    ) -> Result<Response<SetActiveBufferResponse>, Status> {
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let buffer_id = BufferId::from_raw(req.buffer_id as usize);

        // Verify buffer exists
        let exists = session
            .with_state(|state| state.buffer(buffer_id).is_some())
            .await;

        if !exists {
            return Err(Status::not_found(format!("Buffer {} not found", req.buffer_id)));
        }

        // Set per-client active_buffer (#471)
        if let Some(cid) = client_id {
            session.update_client_state(cid, |state| {
                state.active_buffer = Some(buffer_id);
            });
        }

        Ok(Response::new(SetActiveBufferResponse { ok: true }))
    }

    /// Get the active buffer ID.
    ///
    /// Per-client `active_buffer` (#471): returns the calling client's active buffer.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_active_buffer(
        &self,
        request: Request<GetActiveBufferRequest>,
    ) -> Result<Response<GetActiveBufferResponse>, Status> {
        let client_id = request.extensions().get::<ClientId>().copied();
        let session = self.get_session()?;

        // Per-client active_buffer (#471)
        let buffer_id = client_id.and_then(|cid| {
            session
                .with_clients(|clients| {
                    clients
                        .get(&cid)
                        .and_then(|c| c.state.active_buffer.map(|id| id.as_usize() as u64))
                })
        });

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
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
                TextObjectEngine,
            },
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
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
    async fn test_resize_relays_notification() {
        let registry = test_registry();
        let service = EditorServiceImpl::new(registry.clone(), SessionId::new("test"));

        // Subscribe to notifications before resize
        let session = registry.get(&super::SessionId::new("test")).unwrap();
        let mut rx = session.subscribe_notifications();

        let request = Request::new(ResizeRequest {
            width: 80,
            height: 24,
        });
        let response = service.resize(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);

        // Verify notification was emitted
        let notification = rx.try_recv().unwrap();
        assert_eq!(notification.event_type, "resize_request");
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

    #[tokio::test]
    async fn test_resize_with_authenticated_client() {
        let registry = test_registry();
        let service = EditorServiceImpl::new(registry.clone(), SessionId::new("test"));

        let session = registry.get(&SessionId::new("test")).unwrap();
        let mut rx = session.subscribe_notifications();

        let client_id = ClientId::new(99);
        let mut request = Request::new(ResizeRequest {
            width: 100,
            height: 50,
        });
        request.extensions_mut().insert(client_id);

        let response = service.resize(request).await;
        assert!(response.is_ok());

        // Check the notification payload includes the target client ID
        let notification = rx.try_recv().unwrap();
        assert_eq!(notification.event_type, "resize_request");
        if let Some(reovim_protocol::v2::notification::Payload::ResizeRequest(payload)) =
            notification.payload
        {
            assert_eq!(payload.width, 100);
            assert_eq!(payload.height, 50);
            assert_eq!(payload.target_client_id, 99);
        } else {
            panic!("Expected ResizeRequest payload");
        }
    }
}
