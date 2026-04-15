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
        ListClientsRequest, ListClientsResponse, Notification, PresenceUpdate,
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

    // Get buffer_id from active window
    let buffer_id = client.state.windows.active().and_then(|w| w.buffer_id);

    // (#753) ClientViewState: cursor/selection/mode replaced by domain_state (opaque DomainDatum).
    let view = ProtoViewState {
        buffer_id: buffer_id.map(|id| id.as_usize() as u64),
        domain_state: None, // domain state populated by text-domain driver projections
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
        // (#753) visible_lines/mode replaced by opaque viewport_state (DomainDatum).
        viewport_state: None, // populated by text-domain driver projections
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
pub fn build_presence_updated_notification(presence: &ClientPresence) -> Notification {
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
        presence.buffer_id = session.clients().with_clients(|clients| {
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
        let peers_v2: Vec<ProtoClientInfo> = session.clients().with_clients(|clients| {
            clients
                .values()
                .filter(|c| c.id != client_id) // Exclude self
                .map(to_proto_client_info)
                .collect()
        });

        // Start per-client illuminate tick (#664)
        session.with_state_mut_sync(|state| {
            use reovim_subsys_session::{ClientId as DriverClientId, TickSchedulerHandle};

            if let Some(tick_handle) = state.app.services.get::<TickSchedulerHandle>() {
                tick_handle.start(
                    DriverClientId::new(client_id.as_usize()),
                    "illuminate",
                    std::time::Duration::from_millis(100),
                );
            }
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

        // (#753) Update presence via closure.
        // visible_lines and mode fields replaced by opaque viewport_state (DomainDatum).
        let updated = session.presence().update(client_id, |presence| {
            if let Some(buffer_id) = req.buffer_id {
                presence.buffer_id = Some(buffer_id as usize);
            }
            // viewport_state is opaque; text-domain specific visible_lines/mode removed.
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
        let clients_v2: Vec<ProtoClientInfo> = session
            .clients()
            .with_clients(|c| c.values().map(to_proto_client_info).collect());

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
        if !session.clients().has_client(client_id) {
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
        match session.clients().set_client_relation(client_id, relation) {
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
                    TransitionResult::RequiresDomainSync => {
                        "Domain sync required for this transition".to_string()
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
        match session.clients().set_client_relation(client_id, relation) {
            Ok(()) => Ok(Response::new(SetRelationResponse {
                ok: true,
                error: None,
            })),
            Err(err) => {
                let error_code = match err {
                    TransitionResult::TargetNotFound(_) => ProtoTransitionError::TargetNotFound,
                    TransitionResult::WouldCreateCycle => ProtoTransitionError::WouldCreateCycle,
                    TransitionResult::CannotTargetSelf => ProtoTransitionError::CannotTargetSelf,
                    TransitionResult::RequiresDomainSync => {
                        ProtoTransitionError::RequiresDomainSync
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
#[path = "presence_tests.rs"]
mod tests;
