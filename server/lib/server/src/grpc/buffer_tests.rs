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
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
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
async fn test_get_raw_content_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service = BufferServiceImpl::new(registry, SessionId::new("nonexistent"));

    let request = Request::new(GetRawContentRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    let response = service.get_raw_content(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_raw_content_no_buffer() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetRawContentRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    let response = service.get_raw_content(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_raw_content_with_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("hello\nworld");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetRawContentRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    let response = service.get_raw_content(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.lines.len(), 2);
    assert_eq!(resp.lines[0], "hello");
    assert_eq!(resp.lines[1], "world");
}

#[tokio::test]
async fn test_get_line_count_no_buffer() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetLineCountRequest { buffer_id: None });
    let response = service.get_line_count(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_line_count_with_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("line1\nline2\nline3");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetLineCountRequest { buffer_id: None });
    let response = service.get_line_count(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.line_count, 3);
}

#[tokio::test]
async fn test_list_buffers_empty() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListBuffersRequest {});
    let response = service.list(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.buffers.is_empty());
}

#[tokio::test]
async fn test_list_buffers_with_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("content");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListBuffersRequest {});
    let response = service.list(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffers.len(), 1);
    assert_eq!(resp.buffers[0].line_count, 1);
}

#[tokio::test]
async fn test_open_file_unimplemented() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(OpenFileRequest {
        path: "test.txt".to_string(),
    });
    let response = service.open_file(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn test_write_file_unimplemented() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(WriteFileRequest {
        buffer_id: Some(0),
        path: None,
    });
    let response = service.write_file(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn test_set_content_unimplemented() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(SetContentRequest {
        buffer_id: Some(0),
        content: "test".to_string(),
    });
    let response = service.set_content(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn test_get_raw_content_with_specific_buffer_id() {
    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("alpha\nbeta\ngamma"))
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetRawContentRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        start_line: None,
        end_line: None,
    });
    let response = service.get_raw_content(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.lines.len(), 3);
    assert_eq!(resp.lines[0], "alpha");
    assert_eq!(resp.lines[2], "gamma");
}

#[tokio::test]
async fn test_get_raw_content_with_line_range() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("line0\nline1\nline2\nline3\nline4");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetRawContentRequest {
        buffer_id: None,
        start_line: Some(1),
        end_line: Some(3),
    });
    let response = service.get_raw_content(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.start_line, 1);
    assert_eq!(resp.lines.len(), 2); // lines 1 and 2
    assert_eq!(resp.lines[0], "line1");
    assert_eq!(resp.lines[1], "line2");
}

#[tokio::test]
async fn test_get_raw_content_nonexistent_specific_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    // Create a buffer so there's an active one, but query a different ID
    session
        .with_state_mut(|state| {
            state.create_buffer("content");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetRawContentRequest {
        buffer_id: Some(999),
        start_line: None,
        end_line: None,
    });
    let response = service.get_raw_content(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_annotations_with_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("fn main() {}");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetAnnotationsRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    let response = service.get_annotations(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Currently returns empty annotations
    assert!(resp.annotations.is_empty());
}

#[tokio::test]
async fn test_get_annotations_with_specific_buffer_id() {
    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetAnnotationsRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        start_line: None,
        end_line: None,
    });
    let response = service.get_annotations(request).await;

    assert!(response.is_ok());
}

#[tokio::test]
async fn test_get_annotations_no_buffer() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetAnnotationsRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    let response = service.get_annotations(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_line_count_with_specific_buffer_id() {
    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("a\nb\nc\nd"))
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetLineCountRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
    });
    let response = service.get_line_count(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.line_count, 4);
}

#[tokio::test]
async fn test_get_line_count_nonexistent_buffer() {
    let (registry, _session) = test_registry_with_buffer_manager();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetLineCountRequest {
        buffer_id: Some(999),
    });
    let response = service.get_line_count(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_list_buffers_multiple() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("first");
            state.create_buffer("second\nlines");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListBuffersRequest {});
    let response = service.list(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffers.len(), 2);
}

#[test]
fn test_buffer_service_new() {
    let registry = Arc::new(SessionRegistry::new());
    let service = BufferServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));
    let _ = service;
}

#[tokio::test]
async fn test_list_buffers_with_file_path() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            let id = state.create_buffer("content");
            let buf = state.app.kernel.buffers.get(id).unwrap();
            buf.write()
                .set_file_path(Some("/home/user/hello.rs".to_string()));
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListBuffersRequest {});
    let response = service.list(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffers.len(), 1);
    assert_eq!(resp.buffers[0].name, "hello.rs");
    assert_eq!(resp.buffers[0].path.as_deref(), Some("/home/user/hello.rs"));
}

#[test]
fn test_buffer_service_get_session_not_found() {
    let registry = Arc::new(SessionRegistry::new());
    let service = BufferServiceImpl::new(registry, SessionId::new("nonexistent"));
    let result = service.get_session();
    assert!(result.is_err());
}

#[test]
fn test_buffer_service_get_session_found() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));
    let result = service.get_session();
    assert!(result.is_ok());
}

/// Per-client `active_buffer` (#471): when two buffers exist and a client
/// has `active_buffer` set to the second one, `get_raw_content(None)`
/// should return the second buffer's content, not the first.
#[tokio::test]
async fn test_get_raw_content_uses_client_active_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    // Create two buffers
    let (buf1, buf2) = session
        .with_state_mut(|state| {
            let b1 = state.create_buffer("first buffer");
            let b2 = state.create_buffer("second buffer");
            (b1, b2)
        })
        .await;

    // Register a client with active_buffer pointing to buf2
    let client_id = ClientId::new(42);
    session.add_client(client_id);
    session.update_client_state(client_id, |state| {
        state.active_buffer = Some(buf2);
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    // Request without buffer_id but WITH client_id in extensions
    let mut request = Request::new(GetRawContentRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    request.extensions_mut().insert(client_id);
    let response = service.get_raw_content(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(
        resp.lines[0], "second buffer",
        "Should return client's active buffer, not first"
    );
    assert_eq!(resp.buffer_id, buf2.as_usize() as u64);

    // Without client_id, should fall back to any buffer in list
    let request = Request::new(GetRawContentRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    let response = service.get_raw_content(request).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    let fallback_id = resp.buffer_id;
    assert!(
        fallback_id == buf1.as_usize() as u64 || fallback_id == buf2.as_usize() as u64,
        "Fallback should return a valid buffer, got {fallback_id}"
    );
}
