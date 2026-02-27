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
    use {
        super::*,
        reovim_driver_session::{ExtensionMap, bridges::ExtensionStateBridge},
    };

    struct TestBridge {
        kind_str: &'static str,
        bridge_scope: ExtensionScope,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ExtensionStateBridge for TestBridge {
        fn kind(&self) -> &'static str {
            self.kind_str
        }
        fn scope(&self) -> ExtensionScope {
            self.bridge_scope
        }
        fn snapshot(&self, _: &ExtensionMap) -> Option<serde_json::Value> {
            Some(serde_json::json!({"test": true}))
        }
        fn is_active(&self, _: &ExtensionMap) -> bool {
            true
        }
    }

    fn make_service(bridges: BridgeRegistry) -> (ExtensionServiceImpl, Arc<Session>) {
        let session = Arc::new(Session::new(SessionId::new("test")));
        let sessions = Arc::new(SessionRegistry::new());
        sessions.insert(&session);
        let service =
            ExtensionServiceImpl::new(sessions, SessionId::new("test"), Arc::new(bridges));
        (service, session)
    }

    fn authed_request<T>(body: T, client_id: ClientId) -> Request<T> {
        let mut request = Request::new(body);
        request.extensions_mut().insert(client_id);
        request
    }

    #[test]
    fn test_extension_service_impl_new() {
        let sessions = Arc::new(SessionRegistry::new());
        let session_id = SessionId::new("test");
        let bridges = Arc::new(BridgeRegistry::new());
        let service = ExtensionServiceImpl::new(sessions, session_id, bridges);
        assert!(service.get_session().is_err()); // No session exists
    }

    #[tokio::test]
    async fn test_get_state_unknown_kind() {
        let (service, _) = make_service(BridgeRegistry::new());

        let request = authed_request(
            GetExtensionStateRequest {
                kind: "nonexistent".to_string(),
                client_id: 0,
            },
            ClientId::new(1),
        );
        let result = service.get_state(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_state_client_scope() {
        let mut bridges = BridgeRegistry::new();
        bridges.register(TestBridge {
            kind_str: "test-ext",
            bridge_scope: ExtensionScope::Client,
        });
        let (service, session) = make_service(bridges);

        // Add a client so with_client_extensions can find it
        session.add_client(ClientId::new(1));

        let request = authed_request(
            GetExtensionStateRequest {
                kind: "test-ext".to_string(),
                client_id: 0, // use token client_id
            },
            ClientId::new(1),
        );
        let response = service.get_state(request).await.unwrap().into_inner();

        assert!(response.active);
        assert!(response.data.contains("test"));
    }

    #[tokio::test]
    async fn test_get_state_shared_scope() {
        let mut bridges = BridgeRegistry::new();
        bridges.register(TestBridge {
            kind_str: "shared-ext",
            bridge_scope: ExtensionScope::Shared,
        });
        let (service, _) = make_service(bridges);

        let request = authed_request(
            GetExtensionStateRequest {
                kind: "shared-ext".to_string(),
                client_id: 1,
            },
            ClientId::new(1),
        );
        let response = service.get_state(request).await.unwrap().into_inner();

        assert!(!response.active);
        assert!(response.data.is_empty());
    }

    #[tokio::test]
    async fn test_list_extensions_with_bridges() {
        let mut bridges = BridgeRegistry::new();
        bridges.register(TestBridge {
            kind_str: "alpha",
            bridge_scope: ExtensionScope::Client,
        });
        bridges.register(TestBridge {
            kind_str: "beta",
            bridge_scope: ExtensionScope::Shared,
        });
        let (service, _) = make_service(bridges);

        let request = Request::new(ListExtensionsRequest {});
        let response = service.list_extensions(request).await.unwrap().into_inner();

        assert_eq!(response.extensions.len(), 2);
        let kinds: Vec<&str> = response
            .extensions
            .iter()
            .map(|e| e.kind.as_str())
            .collect();
        assert!(kinds.contains(&"alpha"));
        assert!(kinds.contains(&"beta"));
    }

    #[tokio::test]
    async fn test_list_extensions_empty() {
        let (service, _) = make_service(BridgeRegistry::new());

        let request = Request::new(ListExtensionsRequest {});
        let response = service.list_extensions(request).await.unwrap().into_inner();

        assert!(response.extensions.is_empty());
    }
}
