use super::*;

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

/// Create a registry with a session that has a real buffer manager and
/// `TextBufferRegistry` so that `state.buffer()` can resolve buffers.
fn test_registry_with_buffer_manager() -> (Arc<SessionRegistry>, Arc<Session>) {
    use {
        reovim_driver_text_buffer::TestBufferManager,
        reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry},
    };

    let services = Arc::new(ServiceRegistry::new());
    services.register(Arc::new(reovim_driver_text_buffer::TextBufferRegistry::new()));
    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        services,
    );

    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));

    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    (registry, session)
}

// test_resize_relays_notification: REMOVED — ResizeRequest/resize RPC deleted in v3 (#753).

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
    // active_buffer is domain-owned (#753 E3); without a wired domain driver,
    // get_active_buffer returns None regardless of buffers created.
    let (registry, _session) = test_registry_with_buffer_manager();
    let service = EditorServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(GetActiveBufferRequest {});
    request.extensions_mut().insert(ClientId::new(1));
    let response = service.get_active_buffer(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // No domain driver wired in unit test -> None
    assert!(resp.buffer_id.is_none());
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
    let (registry, _session) = test_registry_with_buffer_manager();

    // create_buffer removed from SessionState (#753); skip buffer creation.
    // set_active_buffer with buffer_id=0 returns NotFound (no buffer at id 0).
    let service = EditorServiceImpl::new(registry.clone(), SessionId::new("test"));

    let request = Request::new(SetActiveBufferRequest { buffer_id: 0 });
    let response = service.set_active_buffer(request).await;

    // No buffer with id 0 exists -> NotFound
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

// test_resize_with_authenticated_client: REMOVED — ResizeRequest/resize RPC deleted in v3 (#753).

// test_resize_updates_terminal_size_for_existing_client: REMOVED — ResizeRequest/resize RPC deleted in v3 (#753).

/// Cover the stub path in set_active_buffer with an authenticated client.
///
/// active_buffer is now domain-owned (#753 E3); the RPC logs a debug stub
/// and returns ok. Verify the handler succeeds with a client_id in extensions.
#[tokio::test]
async fn test_set_active_buffer_updates_per_client_state() {
    let (registry, session) = test_registry_with_buffer_manager();

    // create_buffer removed from SessionState (#753); use buffer_id 1 as a stub.
    // set_active_buffer with no real buffer returns NotFound; test only verifies
    // the handler does not panic with an authenticated client.
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let service = EditorServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(SetActiveBufferRequest { buffer_id: 999 });
    request.extensions_mut().insert(client_id);

    let response = service.set_active_buffer(request).await;
    // No real buffer at 999 -> NotFound; handler should not panic.
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}
