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
    let buf_id = session
        .with_state_mut(|state| state.create_buffer("test content"))
        .await;

    // Per-client active_buffer (#471): add a client and set their active buffer
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    session.clients().with_clients_mut(|clients| {
        if let Some(client) = clients.get_mut(&client_id) {
            client.state.active_buffer = Some(buf_id);
        }
    });

    let service = EditorServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(GetActiveBufferRequest {});
    request.extensions_mut().insert(client_id);
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

/// Cover lines 107-109: the closure body inside `update_client_state` for resize.
///
/// The existing `test_resize_with_authenticated_client` passes a `ClientId` in
/// request extensions but does NOT add the client to the session, so
/// `update_client_state` exits early without ever running the closure.  Here we
/// add the client first so the closure executes.
#[tokio::test]
async fn test_resize_updates_terminal_size_for_existing_client() {
    let (registry, session) = test_registry_with_buffer_manager();

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let service = EditorServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(ResizeRequest {
        width: 120,
        height: 40,
    });
    request.extensions_mut().insert(client_id);

    let response = service.resize(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);

    // Verify the closure body ran: terminal_size was updated in per-client state.
    let updated = session
        .clients()
        .with_clients(|clients| clients.get(&client_id).map(|c| c.state.terminal_size));
    assert_eq!(updated, Some((120u16, 40u16)));
}

/// Cover lines 152-154: the closure body inside `update_client_state` for
/// `set_active_buffer`.
///
/// Same pattern as resize: the existing `test_set_active_buffer_success` calls
/// the handler without a `ClientId` in extensions, so the closure never runs.
/// Here we add the client and inject its ID.
#[tokio::test]
async fn test_set_active_buffer_updates_per_client_state() {
    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("test content"))
        .await;

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let service = EditorServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let mut request = Request::new(SetActiveBufferRequest {
        buffer_id: buffer_id.as_usize() as u64,
    });
    request.extensions_mut().insert(client_id);

    let response = service.set_active_buffer(request).await;
    assert!(response.is_ok());
    assert!(response.unwrap().into_inner().ok);

    // Verify the closure body ran: active_buffer was set for this client.
    let active = session
        .clients()
        .with_clients(|clients| clients.get(&client_id).and_then(|c| c.state.active_buffer));
    assert_eq!(active, Some(buffer_id));
}
