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
        ClientInfo as ProtoClientInfo, ClientMetadata as ProtoClientMetadata,
        ClientPresence as ProtoClientPresence, ClientRelation as ProtoClientRelation,
        ClientRelationType as ProtoRelationType, ClientRole as ProtoRole,
        ClientViewState as ProtoViewState, JoinRequest, JoinResponse, LeaveRequest, LeaveResponse,
        LineRange, ListClientsRequest, ListClientsResponse, Notification, Position, PresenceUpdate,
        SetRelationRequest, SetRelationResponse, SetRoleRequest, SetRoleResponse,
        SetSyncModeRequest, SetSyncModeResponse, StreamPresenceRequest, SyncMode as ProtoSyncMode,
        TransitionError as ProtoTransitionError, UpdatePresenceRequest, UpdatePresenceResponse,
        notification::Payload, presence_service_server::PresenceService, presence_update::Update,
    },
    tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
    tonic::{Request, Response, Status},
};

use crate::{
    grpc::auth::require_client_id,
    session::{
        Client, ClientId, ClientPresence, ClientRelation, Session, SessionId, SessionRegistry,
        SyncMode, TokenRegistry, TransitionResult,
    },
};

use reovim_kernel::api::BufferId;

/// Get current Unix timestamp in milliseconds.
#[allow(clippy::cast_possible_truncation)]
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH")
        .as_millis() as u64
}

/// Convert internal `Client` to new `ClientInfo` protobuf format (#480).
pub fn to_proto_client_info(client: &Client) -> ProtoClientInfo {
    let relation = client.relation.map(|r| match r {
        ClientRelation::Following { target } => ProtoClientRelation {
            r#type: ProtoRelationType::RelationTypeFollowing as i32,
            target_id: target.as_usize() as u64,
        },
        ClientRelation::Sharing { with } => ProtoClientRelation {
            r#type: ProtoRelationType::RelationTypeSharing as i32,
            target_id: with.as_usize() as u64,
        },
    });

    // Get cursor from active window
    let cursor = client
        .state
        .windows
        .active()
        .map(|w| Position {
            line: w.cursor.line as u64,
            column: w.cursor.column as u64,
        })
        .unwrap_or_default();

    // Get buffer_id from active window
    let buffer_id = client.state.windows.active().and_then(|w| w.buffer_id);

    let view = ProtoViewState {
        mode: client.state.mode_stack.current().name().to_string(),
        cursor: Some(cursor),
        buffer_id: buffer_id.map(|id| id.as_usize() as u64),
        selection: None, // TODO: convert selection if present
    };

    let metadata = ProtoClientMetadata {
        client_type: client.metadata.client_type.clone(),
        display_name: client.metadata.display_name.clone(),
        joined_at_ms: client.metadata.joined_at_ms,
    };

    ProtoClientInfo {
        id: client.id.as_usize() as u64,
        relation,
        view: Some(view),
        metadata: Some(metadata),
    }
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
        // Phase #479: Use optional field to eliminate ID ambiguity
        buffer_id: presence.buffer_id.map(|id| id as u64),
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
    /// Token registry for session-based authentication (#483).
    tokens: Arc<TokenRegistry>,
}

impl PresenceServiceImpl {
    /// Create a new `PresenceService` with access to the session registry.
    #[must_use]
    pub const fn new(
        sessions: Arc<SessionRegistry>,
        default_session_id: SessionId,
        tokens: Arc<TokenRegistry>,
    ) -> Self {
        Self {
            sessions,
            default_session_id,
            tokens,
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

        // Phase #479: Create per-client state (mode, cursor, windows)
        // This MUST happen before any state queries (get_mode, get_cursor, etc.)
        // The metadata carries client_type and display_name for diagnostics.
        let metadata = crate::session::ClientMetadata::new(&req.client_type, &req.display_name);
        session.add_client_with_metadata(client_id, metadata);

        // Create presence and populate buffer_id from the new client's state.
        // ClientPresence::new() defaults to buffer_id: None, but the client
        // was just assigned a window+buffer by add_client_with_metadata().
        // Without this, PresenceJoined notifications carry buffer_id: None,
        // and existing clients skip rendering the new cursor (different-buffer filter).
        let mut presence = ClientPresence::new(client_id, &req.client_type, &req.display_name);
        presence.buffer_id = session.with_clients(|clients| {
            clients.get(&client_id).and_then(|c| {
                c.state
                    .windows
                    .active()
                    .and_then(|w| w.buffer_id.map(BufferId::as_usize))
            })
        });

        // Add to presence map, get existing peers
        let peers = session.presence().join(presence.clone());

        // Emit notification to all subscribers
        session.emit_notification(build_presence_joined_notification(&presence));

        // Convert peers to protobuf format (legacy)
        let proto_peers: Vec<ProtoClientPresence> = peers.iter().map(to_proto_presence).collect();

        // Convert clients to new ClientInfo format (#480)
        let peers_v2: Vec<ProtoClientInfo> = session.with_clients(|clients| {
            clients
                .values()
                .filter(|c| c.id != client_id) // Exclude self
                .map(to_proto_client_info)
                .collect()
        });

        // Generate session token for this client (#483)
        let token = self.tokens.register(client_id);

        Ok(Response::new(JoinResponse {
            client_id: client_id.as_usize() as u64,
            peers: proto_peers,
            peers_v2,
            session_token: token.to_string(),
        }))
    }

    /// Leave the session.
    ///
    /// Removes client from presence map and emits `presence_left` notification.
    async fn leave(
        &self,
        request: Request<LeaveRequest>,
    ) -> Result<Response<LeaveResponse>, Status> {
        // #483 Phase 5: Token-only authentication
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let _req = request.into_inner(); // LeaveRequest has no fields after #483
        let session = self.get_session()?;

        let client_id = require_client_id(token_client_id)?;

        // Revoke session token (#483)
        self.tokens.revoke_by_client(client_id);

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
        // #483 Phase 5: Token-only authentication
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // #483 Phase 5: Token-only authentication
        let client_id = require_client_id(token_client_id)?;

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
        // #483 Phase 5: Token-only authentication
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // #483 Phase 5: Token-only authentication
        let client_id = require_client_id(token_client_id)?;

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

        // Legacy format
        let clients: Vec<ProtoClientPresence> = session
            .presence()
            .list()
            .iter()
            .map(to_proto_presence)
            .collect();

        // New unified format (#480)
        let clients_v2: Vec<ProtoClientInfo> =
            session.with_clients(|c| c.values().map(to_proto_client_info).collect());

        Ok(Response::new(ListClientsResponse {
            clients,
            clients_v2,
        }))
    }

    /// Set a client's editing role (Phase 11.2, Epic #465).
    ///
    /// Controls input routing:
    /// - Owner: Input goes to own state (independent)
    /// - Follow: Input is ignored (read-only spectator)
    /// - Share: Input goes to owner's state
    ///
    /// **Note**: This RPC uses the old `ClientRole` enum. For new code,
    /// prefer using `set_client_relation()` with `ClientRelation` directly.
    async fn set_role(
        &self,
        request: Request<SetRoleRequest>,
    ) -> Result<Response<SetRoleResponse>, Status> {
        use crate::session::ClientRelation;

        // #483 Phase 5: Token-only authentication
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // #483 Phase 5: Token-only authentication
        let client_id = require_client_id(token_client_id)?;

        // Validate client exists
        if !session.has_client(client_id) {
            return Ok(Response::new(SetRoleResponse {
                ok: false,
                error: Some(format!("Client {client_id} not found")),
            }));
        }

        // Map proto role to ClientRelation (#480: unified model)
        let relation = match req.role() {
            ProtoRole::Owner => None, // Independent
            ProtoRole::Follow => {
                let target_id = req.target_id.ok_or_else(|| {
                    Status::invalid_argument("target_id required for FOLLOW role")
                })?;
                Some(ClientRelation::Following {
                    target: ClientId::new(target_id as usize),
                })
            }
            ProtoRole::Share => {
                let owner_id = req.target_id.ok_or_else(|| {
                    Status::invalid_argument("target_id (owner) required for SHARE role")
                })?;
                Some(ClientRelation::Sharing {
                    with: ClientId::new(owner_id as usize),
                })
            }
        };

        // Set the relation with validation
        match session.set_client_relation(client_id, relation) {
            Ok(()) => Ok(Response::new(SetRoleResponse {
                ok: true,
                error: None,
            })),
            Err(err) => {
                use crate::session::TransitionResult;
                let error_msg = match err {
                    TransitionResult::TargetNotFound(id) => {
                        format!("Target client {} not found", id.as_usize())
                    }
                    TransitionResult::WouldCreateCycle => {
                        "Cannot set relation: would create a cycle".to_string()
                    }
                    TransitionResult::CannotTargetSelf => "Cannot target self".to_string(),
                    TransitionResult::RequiresCursorSync { .. } => {
                        "Cursor sync required for this transition".to_string()
                    }
                    TransitionResult::Ok => unreachable!(),
                };
                Err(Status::failed_precondition(error_msg))
            }
        }
    }

    /// Set client's relation (#480 Client Architecture Unification).
    ///
    /// Unified API for managing client relationships. Replaces the separate
    /// `SetSyncMode` and `SetRole` RPCs with a single validated transition.
    ///
    /// # Arguments
    ///
    /// * `client_id` - Client ID to set relation for
    /// * `relation` - New relation. `None` = independent
    ///
    /// # Returns
    ///
    /// * `ok: true` if relation was set
    /// * `error` - Error code if validation failed
    async fn set_relation(
        &self,
        request: Request<SetRelationRequest>,
    ) -> Result<Response<SetRelationResponse>, Status> {
        // #483 Phase 5: Token-only authentication
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // #483 Phase 5: Token-only authentication
        let client_id = require_client_id(token_client_id)?;

        // Convert proto relation to internal relation
        let relation = req.relation.map(|r| {
            let target_id = ClientId::new(r.target_id as usize);
            match ProtoRelationType::try_from(r.r#type) {
                Ok(ProtoRelationType::RelationTypeFollowing) => {
                    ClientRelation::Following { target: target_id }
                }
                Ok(ProtoRelationType::RelationTypeSharing) => {
                    ClientRelation::Sharing { with: target_id }
                }
                Err(_) => ClientRelation::Following { target: target_id }, // Default to following
            }
        });

        // Set the relation with validation
        match session.set_client_relation(client_id, relation) {
            Ok(()) => Ok(Response::new(SetRelationResponse {
                ok: true,
                error: None,
            })),
            Err(err) => {
                let error_code = match err {
                    TransitionResult::TargetNotFound(_) => ProtoTransitionError::TargetNotFound,
                    TransitionResult::WouldCreateCycle => ProtoTransitionError::WouldCreateCycle,
                    TransitionResult::CannotTargetSelf => ProtoTransitionError::CannotTargetSelf,
                    TransitionResult::RequiresCursorSync { .. } => {
                        ProtoTransitionError::RequiresCursorSync
                    }
                    TransitionResult::Ok => unreachable!(),
                };
                Ok(Response::new(SetRelationResponse {
                    ok: false,
                    error: Some(error_code as i32),
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::session::SessionToken};

    fn test_registry() -> Arc<SessionRegistry> {
        let registry = Arc::new(SessionRegistry::new());
        let session = Arc::new(Session::new(SessionId::new("test")));
        registry.insert(&session);
        registry
    }

    fn test_tokens() -> Arc<TokenRegistry> {
        Arc::new(TokenRegistry::new())
    }

    /// Helper: build a request with token-authenticated `ClientId` in extensions.
    fn authed_request<T>(body: T, client_id: ClientId) -> Request<T> {
        let mut request = Request::new(body);
        request.extensions_mut().insert(client_id);
        request
    }

    #[tokio::test]
    async fn test_join_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service =
            PresenceServiceImpl::new(registry, SessionId::new("nonexistent"), test_tokens());

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
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let request = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let response = service.join(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.client_id > 0);
        assert!(resp.peers.is_empty()); // First client has no peers
        // #483: Join now returns a session token
        assert!(!resp.session_token.is_empty());
        assert_eq!(resp.session_token.len(), 32); // 128-bit hex
    }

    #[tokio::test]
    async fn test_join_returns_peers() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

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
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        // Then leave (token identifies client)
        let leave_req = authed_request(LeaveRequest {}, cid);
        let response = service.leave(leave_req).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_leave_unknown_client() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        // Authed as client 999, which doesn't exist
        let request = authed_request(LeaveRequest {}, ClientId::new(999));
        let response = service.leave(request).await;

        assert!(response.is_ok());
        assert!(!response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_update_presence_success() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        // Join first
        let join_req = Request::new(JoinRequest {
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
        });
        let join_resp = service.join(join_req).await.unwrap().into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        // Update presence (token identifies client)
        let update_req = authed_request(
            UpdatePresenceRequest {
                buffer_id: Some(42),
                visible_lines: Some(LineRange { start: 5, end: 30 }),
                mode: Some("INSERT".to_string()),
            },
            cid,
        );
        let response = service.update_presence(update_req).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_update_presence_unknown_client() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let request = authed_request(
            UpdatePresenceRequest {
                buffer_id: Some(42),
                visible_lines: None,
                mode: None,
            },
            ClientId::new(999),
        );
        let response = service.update_presence(request).await;

        assert!(response.is_err());
        assert_eq!(response.err().unwrap().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_set_sync_mode_independent() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "laptop".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetSyncModeRequest {
                mode: ProtoSyncMode::Independent as i32,
                follow_target: None,
            },
            cid,
        );
        let response = service.set_sync_mode(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_sync_mode_present() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "laptop".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetSyncModeRequest {
                mode: ProtoSyncMode::Present as i32,
                follow_target: None,
            },
            cid,
        );
        let response = service.set_sync_mode(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_sync_mode_follow_success() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let resp1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "presenter".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let presenter_id = resp1.client_id;

        let resp2 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "follower".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let follower_cid = ClientId::new(resp2.client_id as usize);

        let request = authed_request(
            SetSyncModeRequest {
                mode: ProtoSyncMode::Follow as i32,
                follow_target: Some(presenter_id),
            },
            follower_cid,
        );
        let response = service.set_sync_mode(request).await;

        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_sync_mode_follow_missing_target() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "laptop".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetSyncModeRequest {
                mode: ProtoSyncMode::Follow as i32,
                follow_target: None,
            },
            cid,
        );
        let response = service.set_sync_mode(request).await;

        assert!(response.is_err());
        assert_eq!(response.err().unwrap().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_set_sync_mode_follow_invalid_target() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "laptop".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetSyncModeRequest {
                mode: ProtoSyncMode::Follow as i32,
                follow_target: Some(9999),
            },
            cid,
        );
        let response = service.set_sync_mode(request).await;

        assert!(response.is_err());
        assert_eq!(response.err().unwrap().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_list_clients() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

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
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let request = Request::new(StreamPresenceRequest {});
        let response = service.stream_presence(request).await;

        // Should successfully return a stream
        assert!(response.is_ok());
    }

    // ── Token lifecycle tests (#483) ──────────────────────────────────

    #[tokio::test]
    async fn test_join_returns_session_token() {
        let tokens = test_tokens();
        let registry = test_registry();
        let service =
            PresenceServiceImpl::new(registry, SessionId::new("test"), Arc::clone(&tokens));

        let resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        // Token is present and valid
        assert_eq!(resp.session_token.len(), 32);
        // Token resolves in the registry
        let token = SessionToken::from(resp.session_token.as_str());
        assert_eq!(tokens.resolve(&token), Some(ClientId::new(resp.client_id as usize)));
    }

    #[tokio::test]
    async fn test_leave_revokes_token() {
        let tokens = test_tokens();
        let registry = test_registry();
        let service =
            PresenceServiceImpl::new(registry, SessionId::new("test"), Arc::clone(&tokens));

        // Join
        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let token = SessionToken::from(join_resp.session_token.as_str());
        assert!(tokens.resolve(&token).is_some());

        // Leave (token identifies client)
        let cid = ClientId::new(join_resp.client_id as usize);
        service
            .leave(authed_request(LeaveRequest {}, cid))
            .await
            .unwrap();

        // Token is revoked
        assert!(tokens.resolve(&token).is_none());
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_to_proto_client_info_independent() {
        use {
            crate::session::ClientMetadata,
            reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        };

        let mode = ModeId::new(ModuleId::new("test"), "normal");
        let mode_stack = ModeStack::new(mode);
        let metadata = ClientMetadata::default();
        let client = Client::with_mode_stack(ClientId::new(42), metadata, mode_stack);

        let info = to_proto_client_info(&client);
        assert_eq!(info.id, 42);
        assert!(info.relation.is_none()); // Independent
        assert!(info.view.is_some());
        assert_eq!(info.view.as_ref().unwrap().mode, "normal");
        assert!(info.metadata.is_some());
    }

    #[test]
    fn test_to_proto_client_info_following() {
        use {
            crate::session::{ClientMetadata, ClientRelation},
            reovim_driver_session::Window,
            reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        };

        let mode = ModeId::new(ModuleId::new("test"), "normal");
        let mode_stack = ModeStack::new(mode);
        let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(10);
        let window = Window::with_buffer(buffer_id);
        let metadata = ClientMetadata::default();
        let mut client =
            Client::with_mode_stack_and_window(ClientId::new(5), metadata, mode_stack, window);
        client.set_relation_unchecked(Some(ClientRelation::Following {
            target: ClientId::new(1),
        }));

        let info = to_proto_client_info(&client);
        assert_eq!(info.id, 5);
        let rel = info.relation.unwrap();
        assert_eq!(rel.r#type, ProtoRelationType::RelationTypeFollowing as i32);
        assert_eq!(rel.target_id, 1);
        // Client has a window with buffer
        assert!(info.view.as_ref().unwrap().buffer_id.is_some());
    }

    #[test]
    fn test_to_proto_client_info_sharing() {
        use {
            crate::session::{ClientMetadata, ClientRelation},
            reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        };

        let mode = ModeId::new(ModuleId::new("test"), "normal");
        let mode_stack = ModeStack::new(mode);
        let metadata = ClientMetadata::default();
        let mut client = Client::with_mode_stack(ClientId::new(3), metadata, mode_stack);
        client.set_relation_unchecked(Some(ClientRelation::Sharing {
            with: ClientId::new(2),
        }));

        let info = to_proto_client_info(&client);
        let rel = info.relation.unwrap();
        assert_eq!(rel.r#type, ProtoRelationType::RelationTypeSharing as i32);
        assert_eq!(rel.target_id, 2);
    }

    #[test]
    fn test_to_proto_presence_independent() {
        let presence = ClientPresence::new(ClientId::new(1), "tui", "laptop");
        let proto = to_proto_presence(&presence);

        assert_eq!(proto.client_id, 1);
        assert_eq!(proto.client_type, "tui");
        assert_eq!(proto.display_name, "laptop");
        assert_eq!(proto.sync_mode, ProtoSyncMode::Independent as i32);
        assert!(proto.follow_target.is_none());
    }

    #[test]
    fn test_to_proto_presence_follow() {
        let mut presence = ClientPresence::new(ClientId::new(2), "cli", "terminal");
        presence.sync_mode = SyncMode::Follow {
            target: ClientId::new(1),
        };

        let proto = to_proto_presence(&presence);
        assert_eq!(proto.sync_mode, ProtoSyncMode::Follow as i32);
        assert_eq!(proto.follow_target, Some(1));
    }

    #[test]
    fn test_to_proto_presence_present() {
        let mut presence = ClientPresence::new(ClientId::new(3), "web", "browser");
        presence.sync_mode = SyncMode::Present;

        let proto = to_proto_presence(&presence);
        assert_eq!(proto.sync_mode, ProtoSyncMode::Present as i32);
        assert!(proto.follow_target.is_none());
    }

    #[test]
    fn test_notification_to_presence_update_joined() {
        let presence = ClientPresence::new(ClientId::new(1), "tui", "laptop");
        let notification = build_presence_joined_notification(&presence);

        let update = notification_to_presence_update(&notification);
        assert!(update.is_some());
        let u = update.unwrap();
        assert!(matches!(u.update, Some(Update::Joined(_))));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_notification_to_presence_update_left() {
        let notification = build_presence_left_notification(ClientId::new(5), "laptop");
        let update = notification_to_presence_update(&notification);
        assert!(update.is_some());
        let u = update.unwrap();
        if let Some(Update::Left(id)) = u.update {
            assert_eq!(id, 5);
        } else {
            panic!("Expected Left update");
        }
    }

    #[test]
    fn test_notification_to_presence_update_updated() {
        let presence = ClientPresence::new(ClientId::new(3), "cli", "term");
        let notification = build_presence_updated_notification(&presence);
        let update = notification_to_presence_update(&notification);
        assert!(update.is_some());
        assert!(matches!(update.unwrap().update, Some(Update::Updated(_))));
    }

    #[test]
    fn test_notification_to_presence_update_non_presence() {
        let notification = Notification {
            event_type: "mode_changed".to_string(),
            timestamp_ms: 0,
            payload: None,
        };
        let update = notification_to_presence_update(&notification);
        assert!(update.is_none());
    }

    #[tokio::test]
    async fn test_set_role_owner() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "laptop".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetRoleRequest {
                role: ProtoRole::Owner as i32,
                target_id: None,
            },
            cid,
        );
        let response = service.set_role(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_role_follow() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "owner".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        let r2 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "follower".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let follower_cid = ClientId::new(r2.client_id as usize);

        let request = authed_request(
            SetRoleRequest {
                role: ProtoRole::Follow as i32,
                target_id: Some(r1.client_id),
            },
            follower_cid,
        );
        let response = service.set_role(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_role_share() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "owner".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        let r2 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "sharer".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let sharer_cid = ClientId::new(r2.client_id as usize);

        let request = authed_request(
            SetRoleRequest {
                role: ProtoRole::Share as i32,
                target_id: Some(r1.client_id),
            },
            sharer_cid,
        );
        let response = service.set_role(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_role_follow_missing_target_id() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetRoleRequest {
                role: ProtoRole::Follow as i32,
                target_id: None, // Missing target!
            },
            cid,
        );
        let response = service.set_role(request).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_set_role_client_not_found() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let request = authed_request(
            SetRoleRequest {
                role: ProtoRole::Owner as i32,
                target_id: None,
            },
            ClientId::new(999),
        );
        let response = service.set_role(request).await;
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok);
        assert!(resp.error.is_some());
    }

    #[tokio::test]
    async fn test_set_role_follow_target_not_found() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetRoleRequest {
                role: ProtoRole::Follow as i32,
                target_id: Some(9999), // Non-existent target
            },
            cid,
        );
        let response = service.set_role(request).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn test_set_relation_following() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "target".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        let r2 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "follower".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let follower_cid = ClientId::new(r2.client_id as usize);

        let request = authed_request(
            SetRelationRequest {
                relation: Some(ProtoClientRelation {
                    r#type: ProtoRelationType::RelationTypeFollowing as i32,
                    target_id: r1.client_id,
                }),
            },
            follower_cid,
        );
        let response = service.set_relation(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_relation_sharing() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "owner".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        let r2 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "sharer".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let sharer_cid = ClientId::new(r2.client_id as usize);

        let request = authed_request(
            SetRelationRequest {
                relation: Some(ProtoClientRelation {
                    r#type: ProtoRelationType::RelationTypeSharing as i32,
                    target_id: r1.client_id,
                }),
            },
            sharer_cid,
        );
        let response = service.set_relation(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_relation_independent() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(r1.client_id as usize);

        // Set to independent (None relation)
        let request = authed_request(SetRelationRequest { relation: None }, cid);
        let response = service.set_relation(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_set_relation_target_not_found() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(r1.client_id as usize);

        let request = authed_request(
            SetRelationRequest {
                relation: Some(ProtoClientRelation {
                    r#type: ProtoRelationType::RelationTypeFollowing as i32,
                    target_id: 99999,
                }),
            },
            cid,
        );
        let response = service.set_relation(request).await;
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok);
        assert!(resp.error.is_some());
    }

    #[tokio::test]
    async fn test_set_relation_self_target() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(r1.client_id as usize);

        // Try to follow self
        let request = authed_request(
            SetRelationRequest {
                relation: Some(ProtoClientRelation {
                    r#type: ProtoRelationType::RelationTypeFollowing as i32,
                    target_id: r1.client_id,
                }),
            },
            cid,
        );
        let response = service.set_relation(request).await;
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok);
        // Should have CannotTargetSelf error
        assert!(resp.error.is_some());
    }

    #[tokio::test]
    async fn test_set_sync_mode_client_not_in_presence_map() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        // Client 42 never joined - not in presence map
        let request = authed_request(
            SetSyncModeRequest {
                mode: ProtoSyncMode::Independent as i32,
                follow_target: None,
            },
            ClientId::new(42),
        );
        let response = service.set_sync_mode(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_set_role_share_missing_target_id() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let join_resp = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let cid = ClientId::new(join_resp.client_id as usize);

        let request = authed_request(
            SetRoleRequest {
                role: ProtoRole::Share as i32,
                target_id: None, // Missing target!
            },
            cid,
        );
        let response = service.set_role(request).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_set_relation_invalid_type_defaults_to_following() {
        let registry = test_registry();
        let service = PresenceServiceImpl::new(registry, SessionId::new("test"), test_tokens());

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "target".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        let r2 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "follower".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let follower_cid = ClientId::new(r2.client_id as usize);

        // Use invalid relation type (999) - should default to Following
        let request = authed_request(
            SetRelationRequest {
                relation: Some(ProtoClientRelation {
                    r#type: 999,
                    target_id: r1.client_id,
                }),
            },
            follower_cid,
        );
        let response = service.set_relation(request).await;
        assert!(response.is_ok());
        assert!(response.unwrap().into_inner().ok);
    }

    #[tokio::test]
    async fn test_each_client_gets_unique_token() {
        let tokens = test_tokens();
        let registry = test_registry();
        let service =
            PresenceServiceImpl::new(registry, SessionId::new("test"), Arc::clone(&tokens));

        let r1 = service
            .join(Request::new(JoinRequest {
                client_type: "tui".to_string(),
                display_name: "a".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        let r2 = service
            .join(Request::new(JoinRequest {
                client_type: "cli".to_string(),
                display_name: "b".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        assert_ne!(r1.session_token, r2.session_token);
        assert_eq!(tokens.len(), 2);
    }
}
