//! `PresenceService` gRPC implementation (Phase 14, Epic #465).
//!
//! Multi-client presence awareness for collaborative editing.
//! Clients can see each other's cursors, follow viewports, and coordinate.
//!
//! # Architecture
//!
//! ```text
//! Client A ──► Join() ──► Server assigns ClientId
//!              │              └─► Emits presence_joined notification
//!              │
//!              ├─► StreamPresence() ──► Receives all presence updates
//!              │
//!              ├─► UpdatePresence() ──► Updates cursor/viewport/mode
//!              │              └─► Emits presence_updated notification
//!              │
//!              └─► Leave() ──► Server removes client
//!                       └─► Emits presence_left notification
//! ```

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]
// gRPC protocol uses u64 for IDs, but internally we use usize. On 64-bit
// platforms (our target), these are equivalent. Truncation on 32-bit is acceptable.
#![allow(clippy::cast_possible_truncation)]

use std::{pin::Pin, sync::Arc, time::SystemTime};

use {
    futures::Stream,
    reovim_protocol::v2::{
        ClientPresence as ProtoClientPresence, ClientRole as ProtoRole, JoinRequest, JoinResponse,
        LeaveRequest, LeaveResponse, LineRange, ListClientsRequest, ListClientsResponse,
        Notification, PresenceUpdate, SetRoleRequest, SetRoleResponse, SetSyncModeRequest,
        SetSyncModeResponse, StreamPresenceRequest, SyncMode as ProtoSyncMode,
        UpdatePresenceRequest, UpdatePresenceResponse, notification::Payload,
        presence_service_server::PresenceService, presence_update::Update,
    },
    tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
    tonic::{Request, Response, Status},
};

use crate::session::{ClientId, ClientPresence, Session, SessionId, SessionRegistry, SyncMode};

/// Get current Unix timestamp in milliseconds.
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Convert internal `ClientPresence` to protobuf format.
///
/// Note: cursor is no longer included in proto (Phase 14, #471).
/// Cursor tracking moved to `CursorMoved` notifications with `client_id`.
fn to_proto_presence(presence: &ClientPresence) -> ProtoClientPresence {
    let (sync_mode, follow_target) = match presence.sync_mode {
        SyncMode::Independent => (ProtoSyncMode::Independent as i32, None),
        SyncMode::Follow { target } => {
            (ProtoSyncMode::Follow as i32, Some(target.as_usize() as u64))
        }
        SyncMode::Present => (ProtoSyncMode::Present as i32, None),
    };

    ProtoClientPresence {
        client_id: presence.client_id.as_usize() as u64,
        client_type: presence.client_type.clone(),
        display_name: presence.display_name.clone(),
        buffer_id: presence.buffer_id.unwrap_or(0) as u64,
        // cursor field removed - now tracked via CursorMoved with client_id
        visible_lines: Some(LineRange {
            start: presence.visible_lines.0 as u64,
            end: presence.visible_lines.1 as u64,
        }),
        mode: presence.mode.clone(),
        sync_mode,
        follow_target,
        joined_at_ms: presence.joined_at_ms(),
    }
}

/// Build `presence_joined` notification.
fn build_presence_joined_notification(presence: &ClientPresence) -> Notification {
    use reovim_protocol::v2::PresenceJoinedPayload;

    Notification {
        event_type: "presence_joined".to_string(),
        timestamp_ms: current_timestamp_ms(),
        payload: Some(Payload::PresenceJoined(PresenceJoinedPayload {
            client: Some(to_proto_presence(presence)),
        })),
    }
}

/// Build `presence_left` notification.
fn build_presence_left_notification(client_id: ClientId, display_name: &str) -> Notification {
    use reovim_protocol::v2::PresenceLeftPayload;

    Notification {
        event_type: "presence_left".to_string(),
        timestamp_ms: current_timestamp_ms(),
        payload: Some(Payload::PresenceLeft(PresenceLeftPayload {
            client_id: client_id.as_usize() as u64,
            display_name: display_name.to_string(),
        })),
    }
}

/// Build `presence_updated` notification.
fn build_presence_updated_notification(presence: &ClientPresence) -> Notification {
    use reovim_protocol::v2::PresenceUpdatedPayload;

    Notification {
        event_type: "presence_updated".to_string(),
        timestamp_ms: current_timestamp_ms(),
        payload: Some(Payload::PresenceUpdated(PresenceUpdatedPayload {
            client: Some(to_proto_presence(presence)),
        })),
    }
}

/// Convert notification to `PresenceUpdate` for streaming.
///
/// Returns `None` if the notification is not a presence-related event.
fn notification_to_presence_update(notification: &Notification) -> Option<PresenceUpdate> {
    match &notification.payload {
        Some(Payload::PresenceJoined(payload)) => Some(PresenceUpdate {
            update: Some(Update::Joined(payload.client.clone()?)),
        }),
        Some(Payload::PresenceUpdated(payload)) => Some(PresenceUpdate {
            update: Some(Update::Updated(payload.client.clone()?)),
        }),
        Some(Payload::PresenceLeft(payload)) => Some(PresenceUpdate {
            update: Some(Update::Left(payload.client_id)),
        }),
        _ => None,
    }
}

/// gRPC `PresenceService` implementation.
///
/// Provides multi-client presence awareness:
/// - Join/leave session
/// - Stream presence updates in real-time
/// - Update cursor/viewport/mode
/// - Set sync mode (follow, present)
pub struct PresenceServiceImpl {
    /// Shared session registry for client ID generation.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
}

impl PresenceServiceImpl {
    /// Create a new `PresenceService` with access to the session registry.
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
impl PresenceService for PresenceServiceImpl {
    /// Join the session and receive assigned client ID + peer list.
    ///
    /// # Flow
    ///
    /// 1. Server generates unique `ClientId` via `SessionRegistry::next_client_id()`
    /// 2. Creates `ClientPresence` with provided type/name
    /// 3. Adds to `PresenceMap`, collecting existing peers
    /// 4. Emits `presence_joined` notification to all subscribers
    /// 5. Returns assigned ID and peer list to caller
    async fn join(&self, request: Request<JoinRequest>) -> Result<Response<JoinResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // Generate unique client ID
        let client_id = self.sessions.next_client_id();

        // Create presence with default state
        let presence = ClientPresence::new(client_id, &req.client_type, &req.display_name);

        // Add to presence map, get existing peers
        let peers = session.presence().join(presence.clone());

        // Emit notification to all subscribers
        session.emit_notification(build_presence_joined_notification(&presence));

        // Convert peers to protobuf format
        let proto_peers: Vec<ProtoClientPresence> = peers.iter().map(to_proto_presence).collect();

        Ok(Response::new(JoinResponse {
            client_id: client_id.as_usize() as u64,
            peers: proto_peers,
        }))
    }

    /// Leave the session.
    ///
    /// Removes client from presence map and emits `presence_left` notification.
    async fn leave(
        &self,
        request: Request<LeaveRequest>,
    ) -> Result<Response<LeaveResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;
        let client_id = ClientId::new(req.client_id as usize);

        // Remove from presence map
        if let Some(presence) = session.presence().leave(client_id) {
            // Emit notification
            session.emit_notification(build_presence_left_notification(
                client_id,
                &presence.display_name,
            ));
            Ok(Response::new(LeaveResponse { ok: true }))
        } else {
            // Client not found - still return ok: false (not an error)
            Ok(Response::new(LeaveResponse { ok: false }))
        }
    }

    /// Stream type for `StreamPresence` RPC.
    type StreamPresenceStream =
        Pin<Box<dyn Stream<Item = Result<PresenceUpdate, Status>> + Send + 'static>>;

    /// Stream presence updates in real-time.
    ///
    /// Subscribes to the session's notification broadcast and filters
    /// for presence-related events (joined, updated, left).
    ///
    /// # Disconnect Handling
    ///
    /// Clients are expected to call `Leave()` for clean disconnect.
    /// The stream drop does NOT automatically remove the client from
    /// presence map because `StreamPresenceRequest` doesn't include
    /// `client_id` (by design - streaming is separate from presence lifecycle).
    ///
    /// Future enhancement: Could add optional `client_id` to request or
    /// use gRPC metadata for automatic cleanup on stream drop.
    async fn stream_presence(
        &self,
        _request: Request<StreamPresenceRequest>,
    ) -> Result<Response<Self::StreamPresenceStream>, Status> {
        let session = self.get_session()?;

        // Get the notification receiver from the session
        let rx = session.subscribe_notifications();

        // Create stream from broadcast receiver
        let stream = BroadcastStream::new(rx);

        // Map and filter the stream for presence events only
        let output_stream = async_stream::stream! {
            let mut stream = stream;
            while let Some(result) = futures::StreamExt::next(&mut stream).await {
                match result {
                    Ok(notification) => {
                        // Convert to PresenceUpdate if it's a presence event
                        if let Some(update) = notification_to_presence_update(&notification) {
                            yield Ok(update);
                        }
                    }
                    Err(BroadcastStreamRecvError::Lagged(n)) => {
                        // Client fell behind, log and continue
                        tracing::warn!(lagged = n, "Presence stream subscriber lagged behind");
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(output_stream)))
    }

    /// Update client's presence state (cursor, viewport, mode).
    ///
    /// Only provided fields are updated; unset fields retain current values.
    async fn update_presence(
        &self,
        request: Request<UpdatePresenceRequest>,
    ) -> Result<Response<UpdatePresenceResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;
        let client_id = ClientId::new(req.client_id as usize);

        // Update presence via closure
        // Note: cursor field removed from request (Phase 14, #471) - now tracked via CursorMoved
        let updated = session.presence().update(client_id, |presence| {
            if let Some(buffer_id) = req.buffer_id {
                presence.buffer_id = Some(buffer_id as usize);
            }
            // cursor field removed - tracked via CursorMoved with client_id
            if let Some(visible_lines) = &req.visible_lines {
                presence.visible_lines = (visible_lines.start as usize, visible_lines.end as usize);
            }
            if let Some(mode) = &req.mode {
                presence.mode.clone_from(mode);
            }
        });

        updated.map_or_else(
            || Err(Status::not_found(format!("Client {client_id} not found in presence map"))),
            |presence| {
                session.emit_notification(build_presence_updated_notification(&presence));
                Ok(Response::new(UpdatePresenceResponse { ok: true }))
            },
        )
    }

    /// Set client's sync mode (independent, follow, present).
    async fn set_sync_mode(
        &self,
        request: Request<SetSyncModeRequest>,
    ) -> Result<Response<SetSyncModeResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;
        let client_id = ClientId::new(req.client_id as usize);

        // Parse sync mode
        let sync_mode = match ProtoSyncMode::try_from(req.mode) {
            Ok(ProtoSyncMode::Independent) => SyncMode::Independent,
            Ok(ProtoSyncMode::Present) => SyncMode::Present,
            Ok(ProtoSyncMode::Follow) => {
                // FOLLOW requires a target
                let target = req.follow_target.ok_or_else(|| {
                    Status::invalid_argument("follow_target is required for FOLLOW mode")
                })?;
                let target_id = ClientId::new(target as usize);

                // Verify target exists
                if !session.presence().contains(target_id) {
                    return Err(Status::invalid_argument(format!(
                        "Follow target {target_id} does not exist"
                    )));
                }

                SyncMode::Follow { target: target_id }
            }
            Err(_) => {
                return Err(Status::invalid_argument(format!("Invalid sync mode: {}", req.mode)));
            }
        };

        // Update presence
        let updated = session.presence().update(client_id, |presence| {
            presence.sync_mode = sync_mode;
        });

        updated.map_or_else(
            || Err(Status::not_found(format!("Client {client_id} not found in presence map"))),
            |presence| {
                session.emit_notification(build_presence_updated_notification(&presence));
                Ok(Response::new(SetSyncModeResponse { ok: true }))
            },
        )
    }

    /// List all currently connected clients.
    async fn list_clients(
        &self,
        _request: Request<ListClientsRequest>,
    ) -> Result<Response<ListClientsResponse>, Status> {
        let session = self.get_session()?;

        let clients: Vec<ProtoClientPresence> = session
            .presence()
            .list()
            .iter()
            .map(to_proto_presence)
            .collect();

        Ok(Response::new(ListClientsResponse { clients }))
    }

    /// Set a client's editing role (Phase 11.2, Epic #465).
    ///
    /// Controls input routing:
    /// - Owner: Input goes to own state
    /// - Follow: Input is ignored (read-only spectator)
    /// - Share: Input goes to owner's state
    async fn set_role(
        &self,
        request: Request<SetRoleRequest>,
    ) -> Result<Response<SetRoleResponse>, Status> {
        use crate::session::Client as ClientEnum;

        let session = self.get_session()?;
        let req = request.into_inner();
        let client_id = ClientId::new(req.client_id as usize);

        // Validate client exists
        if !session.has_client(client_id) {
            return Ok(Response::new(SetRoleResponse {
                ok: false,
                error: Some(format!("Client {client_id} not found")),
            }));
        }

        // Map proto role to internal Client enum
        let role = match req.role() {
            ProtoRole::Owner => ClientEnum::new_owner(),
            ProtoRole::Follow => {
                let target_id = req.target_id.ok_or_else(|| {
                    Status::invalid_argument("target_id required for FOLLOW role")
                })?;
                ClientEnum::follow(ClientId::new(target_id as usize))
            }
            ProtoRole::Share => {
                let owner_id = req.target_id.ok_or_else(|| {
                    Status::invalid_argument("target_id (owner) required for SHARE role")
                })?;
                ClientEnum::share(ClientId::new(owner_id as usize))
            }
        };

        // Set the role
        session.set_client_role(client_id, role);

        Ok(Response::new(SetRoleResponse {
            ok: true,
            error: None,
        }))
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

    #[tokio::test]
    async fn test_join_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = PresenceServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let response = service.join(request).await;

        assert!(response.is_err());
        let err = response.err().unwrap();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_join_success() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let response = service.join(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.client_id > 0);
        assert!(resp.peers.is_empty()); // First client has no peers
    }

    #[tokio::test]
    async fn test_join_returns_peers() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // First client joins
        let request1 = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let _ = service.join(request1).await.unwrap();

        // Second client joins
        let request2 = Request::new(JoinRequest {
            client_type: "android".to_string(),
            display_name: "phone".to_string(),
        });
        let response = service.join(request2).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.peers.len(), 1);
        assert_eq!(resp.peers[0].display_name, "laptop");
    }

    #[tokio::test]
    async fn test_leave_success() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let client_id = join_resp.client_id;

        // Then leave
        let leave_req = Request::new(LeaveRequest { client_id });
        let response = service.leave(leave_req).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_leave_unknown_client() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(LeaveRequest { client_id: 999 });
        let response = service.leave(request).await;

        assert!(response.is_ok());
        assert!(!response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_update_presence_success() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let client_id = join_resp.client_id;

        // Update presence
        // Note: cursor field removed (Phase 14, #471) - now tracked via CursorMoved
        let update_req = Request::new(UpdatePresenceRequest {
            client_id,
            buffer_id: Some(42),
            visible_lines: Some(LineRange { start: 5, end: 30 }),
            mode: Some("INSERT".to_string()),
        });
        let response = service.update_presence(update_req).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_update_presence_unknown_client() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(UpdatePresenceRequest {
            client_id: 999,
            buffer_id: Some(42),
            visible_lines: None,
            mode: None,
        });
        let response = service.update_presence(request).await;

        assert!(response.is_err());
        assert_eq!(response.err().unwrap().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_set_sync_mode_independent() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let client_id = join_resp.client_id;

        // Set sync mode
        let request = Request::new(SetSyncModeRequest {
            client_id,
            mode: ProtoSyncMode::Independent as i32,
            follow_target: None,
        });
        let response = service.set_sync_mode(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_sync_mode_present() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let client_id = join_resp.client_id;

        // Set sync mode to PRESENT
        let request = Request::new(SetSyncModeRequest {
            client_id,
            mode: ProtoSyncMode::Present as i32,
            follow_target: None,
        });
        let response = service.set_sync_mode(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_sync_mode_follow_success() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Client 1 joins as presenter
        let join_req1 = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "presenter".to_string(),
        });
        let join_resp1 = service.join(join_req1).await.unwrap().into_inner();
        let presenter_id = join_resp1.client_id;

        // Client 2 joins and follows client 1
        let join_req2 = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "follower".to_string(),
        });
        let join_resp2 = service.join(join_req2).await.unwrap().into_inner();
        let follower_id = join_resp2.client_id;

        // Set sync mode to FOLLOW
        let request = Request::new(SetSyncModeRequest {
            client_id: follower_id,
            mode: ProtoSyncMode::Follow as i32,
            follow_target: Some(presenter_id),
        });
        let response = service.set_sync_mode(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_sync_mode_follow_missing_target() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let client_id = join_resp.client_id;

        // Set FOLLOW mode without target
        let request = Request::new(SetSyncModeRequest {
            client_id,
            mode: ProtoSyncMode::Follow as i32,
            follow_target: None,
        });
        let response = service.set_sync_mode(request).await;

        assert!(response.is_err());
        assert_eq!(response.err().unwrap().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_set_sync_mode_follow_invalid_target() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let client_id = join_resp.client_id;

        // Set FOLLOW mode with non-existent target
        let request = Request::new(SetSyncModeRequest {
            client_id,
            mode: ProtoSyncMode::Follow as i32,
            follow_target: Some(9999),
        });
        let response = service.set_sync_mode(request).await;

        assert!(response.is_err());
        assert_eq!(response.err().unwrap().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_list_clients() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        // Initially empty
        let request = Request::new(ListClientsRequest {});
        let response = service.list_clients(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().clients.is_empty());

        // Join two clients
        let _ = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "laptop".to_string(),
            }))
            .await
            .unwrap();
        let _ = service
            .join(Request::new(JoinRequest {
                client_type: "android".to_string(),
                display_name: "phone".to_string(),
            }))
            .await
            .unwrap();

        // Now should have two clients
        let request = Request::new(ListClientsRequest {});
        let response = service.list_clients(request).await;
        assert!(response.is_ok());
        let clients = response.unwrap().into_inner().clients;
        assert_eq!(clients.len(), 2);
    }

    #[tokio::test]
    async fn test_stream_presence_returns_stream() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(StreamPresenceRequest {});
        let response = service.stream_presence(request).await;

        // Should successfully return a stream
        assert!(response.is_ok());
    }
}
