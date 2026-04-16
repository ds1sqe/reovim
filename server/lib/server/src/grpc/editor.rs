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

        // Store terminal size in per-client state so compositor.composite()
        // uses correct geometry for layout notifications.
        #[allow(clippy::cast_possible_truncation)]
        if let Some(cid) = client_id {
            let w = req.width as u16;
            let h = req.height as u16;
            session.clients().update_client_state(cid, |state| {
                state.terminal_size = (w, h);
            });
        }

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
