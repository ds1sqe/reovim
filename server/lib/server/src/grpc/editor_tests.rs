use super::*;

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

/// Create a registry with a session that has a real buffer manager.
fn test_registry_with_buffer_manager() -> (Arc<SessionRegistry>, Arc<Session>) {
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, OptionRegistry, ServiceRegistry,
        },
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
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
    session.with_clients_mut(|clients| {
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
