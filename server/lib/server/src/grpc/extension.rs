//! `ExtensionService` gRPC implementation (#514).
//!
//! Enables clients to query extension state (cmdline, which-key, etc.)
//! and discover registered extensions.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]
// gRPC protocol uses u64 for IDs, internally we use usize.
#![allow(clippy::cast_possible_truncation)]

use std::sync::Arc;

use {
    reovim_driver_session::bridges::{BridgeRegistry, ExtensionScope},
    reovim_protocol::v2::{
        ExtensionInfo, GetExtensionStateRequest, GetExtensionStateResponse, ListExtensionsRequest,
        ListExtensionsResponse, extension_service_server::ExtensionService,
    },
    tonic::{Request, Response, Status},
};

use crate::{
    grpc::auth::require_client_id,
    session::{ClientId, Session, SessionId, SessionRegistry},
};

/// gRPC `ExtensionService` implementation.
///
/// Provides extension state queries and discovery via registered bridges.
pub struct ExtensionServiceImpl {
    sessions: Arc<SessionRegistry>,
    default_session_id: SessionId,
    bridges: Arc<BridgeRegistry>,
}

impl ExtensionServiceImpl {
    /// Create a new `ExtensionService`.
    #[must_use]
    pub const fn new(
        sessions: Arc<SessionRegistry>,
        default_session_id: SessionId,
        bridges: Arc<BridgeRegistry>,
    ) -> Self {
        Self {
            sessions,
            default_session_id,
            bridges,
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
impl ExtensionService for ExtensionServiceImpl {
    async fn get_state(
        &self,
        request: Request<GetExtensionStateRequest>,
    ) -> Result<Response<GetExtensionStateResponse>, Status> {
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // Look up bridge
        let bridge = self
            .bridges
            .get(&req.kind)
            .ok_or_else(|| Status::not_found(format!("Unknown extension kind: {}", req.kind)))?;

        // Compute (active, snapshot) in one pass based on scope
        let (active, snapshot) = match bridge.scope() {
            ExtensionScope::Client => {
                let client_id = if req.client_id > 0 {
                    ClientId::new(req.client_id as usize)
                } else {
                    require_client_id(token_client_id)?
                };
                // Use with_client_extensions to avoid cloning EditingState
                // (EditingState::clone() creates empty ExtensionMap)
                session
                    .with_client_extensions(client_id, |extensions| {
                        let active = bridge.is_active(extensions);
                        let snap = bridge.snapshot(extensions);
                        (active, snap)
                    })
                    .unwrap_or((false, None))
            }
            ExtensionScope::Shared => {
                // Shared extensions are not yet implemented at session level.
                // When a shared ExtensionMap is added to SessionState, route here.
                (false, None)
            }
        };

        Ok(Response::new(GetExtensionStateResponse {
            active,
            data: snapshot.map_or_else(String::new, |v| v.to_string()),
        }))
    }

    async fn list_extensions(
        &self,
        _request: Request<ListExtensionsRequest>,
    ) -> Result<Response<ListExtensionsResponse>, Status> {
        let extensions = self
            .bridges
            .kinds()
            .into_iter()
            .map(|kind| ExtensionInfo {
                kind: kind.to_string(),
                description: String::new(),
                push_supported: true,
                query_supported: true,
            })
            .collect();

        Ok(Response::new(ListExtensionsResponse { extensions }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_service_impl_new() {
        let sessions = Arc::new(SessionRegistry::new());
        let session_id = SessionId::new("test");
        let bridges = Arc::new(BridgeRegistry::new());
        let service = ExtensionServiceImpl::new(sessions, session_id, bridges);
        assert!(service.get_session().is_err()); // No session exists
    }
}
