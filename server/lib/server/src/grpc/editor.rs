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
        GetActiveBufferRequest, GetActiveBufferResponse, QuitRequest, QuitResponse,
        SetActiveBufferRequest, SetActiveBufferResponse, SurfaceChangedRequest,
        SurfaceChangedResponse, editor_service_server::EditorService,
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
    /// Notify server of client surface change (replaces Resize).
    ///
    /// Receives an opaque `SurfaceDescriptorProto` — server reads only the
    /// `kind` field for routing. Relays to TUI via notification.
    async fn surface_changed(
        &self,
        request: Request<SurfaceChangedRequest>,
    ) -> Result<Response<SurfaceChangedResponse>, Status> {
        use reovim_protocol::v2::{Notification, SurfaceChangedPayload, notification::Payload};

        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let surface = req.surface;

        // Relay to TUI via notification
        #[allow(clippy::cast_possible_truncation)]
        let notification = Notification {
            event_type: "surface_changed".to_string(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before UNIX_EPOCH")
                .as_millis() as u64,
            payload: Some(Payload::SurfaceChanged(SurfaceChangedPayload {
                surface,
                target_client_id: client_id.map_or(0, |id| id.as_usize() as u64),
            })),
        };

        session.emit_notification(notification);

        tracing::debug!("SurfaceChanged relayed to TUI");

        Ok(Response::new(SurfaceChangedResponse { ok: true }))
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

        // Verify buffer exists via kernel buffer list (#753 E6: no driver imports)
        let exists = session
            .with_state(|state| state.app.kernel.buffers.list().contains(&buffer_id))
            .await;

        if !exists {
            return Err(Status::not_found(format!("Buffer {} not found", req.buffer_id)));
        }

        // active_buffer is domain-owned (#753 E3).
        // The domain driver manages active_buffer internally.
        // Log the intent so future E5/E6 work can wire domain driver routing.
        if let Some(cid) = client_id {
            tracing::debug!(%cid, buffer_id = %buffer_id, "set_active_buffer: forwarding to domain driver (stub #753 E3)");
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

        // Per-client active_buffer (#471, #753 E2: domain driver query)
        let buffer_id = client_id
            .and_then(|cid| session.active_buffer_for_client(cid))
            .map(|id| id.as_usize() as u64);

        Ok(Response::new(GetActiveBufferResponse { buffer_id }))
    }
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
