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
    let service = ExtensionServiceImpl::new(sessions, SessionId::new("test"), Arc::new(bridges));
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
