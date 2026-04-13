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
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry},
    };

    let services = Arc::new(ServiceRegistry::new());
    services.register(Arc::new(reovim_driver_buffer::TextBufferRegistry::new()));
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

// --- GetCodecViews tests ---

#[tokio::test]
async fn test_get_codec_views_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service = BufferServiceImpl::new(registry, SessionId::new("nonexistent"));

    let request = Request::new(GetCodecViewsRequest { buffer_id: None });
    let response = service.get_codec_views(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_codec_views_no_buffer() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetCodecViewsRequest { buffer_id: None });
    let response = service.get_codec_views(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_codec_views_no_codec_state() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("content");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetCodecViewsRequest { buffer_id: None });
    let response = service.get_codec_views(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_codec_views_with_codec_state() {
    use reovim_driver_codec::{CodecMetadata, CodecSessionState, ContentType};

    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| {
            let bid = state.create_buffer("hex content");
            let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
            codec_state.insert(bid, CodecMetadata::new(ContentType::new("text/utf-8")));
            codec_state.set_active_view(bid, "default".to_string());
            bid
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetCodecViewsRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
    });
    let response = service.get_codec_views(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffer_id, buffer_id.as_usize() as u64);
    assert_eq!(resp.active_view, "default");
    // No factory store registered, so views list is empty (factory not found)
    assert!(resp.views.is_empty());
}

// --- SwitchCodecView tests ---

#[tokio::test]
async fn test_switch_codec_view_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service = BufferServiceImpl::new(registry, SessionId::new("nonexistent"));

    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: None,
        view_name: "hex".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_switch_codec_view_no_codec_state() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("content");
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: None,
        view_name: "hex".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_switch_codec_view_no_raw_bytes() {
    use reovim_driver_codec::{CodecMetadata, CodecSessionState, ContentType};

    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            let bid = state.create_buffer("content");
            let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
            codec_state.insert(bid, CodecMetadata::new(ContentType::new("text/utf-8")));
            // No raw bytes cached
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: None,
        view_name: "hex".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn test_switch_codec_view_no_codec_factory() {
    use reovim_driver_codec::{CodecMetadata, CodecSessionState, ContentType};

    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            let bid = state.create_buffer("content");
            let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
            codec_state.insert(bid, CodecMetadata::new(ContentType::new("text/utf-8")));
            codec_state.set_source(bid, b"raw bytes".to_vec());
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: None,
        view_name: "hex".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
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
    session.clients().update_client_state(client_id, |state| {
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

// =========================================================================
// Per-client active_buffer: get_line_count client_active path (#471)
// =========================================================================

/// When a `ClientId` is in the request extensions and the client has
/// `active_buffer` set, `get_line_count` should use that buffer.
#[tokio::test]
async fn test_get_line_count_uses_client_active_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    let (buf1, buf2) = session
        .with_state_mut(|state| {
            let b1 = state.create_buffer("one");
            let b2 = state.create_buffer("line1\nline2\nline3\nline4");
            (b1, b2)
        })
        .await;

    let client_id = ClientId::new(7);
    session.add_client(client_id);
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buf2);
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(GetLineCountRequest { buffer_id: None });
    request.extensions_mut().insert(client_id);
    let response = service.get_line_count(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.line_count, 4);
    assert_eq!(resp.buffer_id, buf2.as_usize() as u64);

    // Without client_id, falls back to buf1 (first in list).
    let request = Request::new(GetLineCountRequest {
        buffer_id: Some(buf1.as_usize() as u64),
    });
    let response = service.get_line_count(request).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.line_count, 1);
}

// =========================================================================
// Per-client active_buffer: get_annotations client_active path (#471)
// =========================================================================

#[tokio::test]
async fn test_get_annotations_uses_client_active_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    let (_buf1, buf2) = session
        .with_state_mut(|state| {
            let b1 = state.create_buffer("first");
            let b2 = state.create_buffer("second");
            (b1, b2)
        })
        .await;

    let client_id = ClientId::new(8);
    session.add_client(client_id);
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buf2);
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(GetAnnotationsRequest {
        buffer_id: None,
        start_line: None,
        end_line: None,
    });
    request.extensions_mut().insert(client_id);
    let response = service.get_annotations(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffer_id, buf2.as_usize() as u64);
    // Annotations are always empty for now.
    assert!(resp.annotations.is_empty());
}

// =========================================================================
// list_buffers: with codec metadata
// =========================================================================

#[tokio::test]
async fn test_list_buffers_with_codec_metadata() {
    use reovim_driver_codec::{CodecMetadata, CodecSessionState, ContentType};

    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| {
            let bid = state.create_buffer("content");
            let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
            let mut meta = CodecMetadata::new(ContentType::new("text/utf-8"));
            meta.set("line_ending", "lf");
            codec_state.insert(bid, meta);
            bid
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListBuffersRequest {});
    let response = service.list(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffers.len(), 1);

    let buf_info = &resp.buffers[0];
    assert_eq!(buf_info.id, buffer_id.as_usize() as u64);

    // Codec metadata should be populated.
    let codec_meta = buf_info
        .codec_metadata
        .as_ref()
        .expect("codec_metadata present");
    assert_eq!(codec_meta.codec_name, "utf-8");
    assert_eq!(codec_meta.line_ending, Some("lf".to_string()));
    assert!(!codec_meta.has_bom);
}

#[tokio::test]
async fn test_list_buffers_with_bom_metadata() {
    use reovim_driver_codec::{CodecMetadata, CodecSessionState, ContentType};

    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            let bid = state.create_buffer("bom content");
            let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
            let mut meta = CodecMetadata::new(ContentType::new("text/utf-8-bom"));
            meta.set("bom", "true");
            codec_state.insert(bid, meta);
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListBuffersRequest {});
    let response = service.list(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffers.len(), 1);

    let codec_meta = resp.buffers[0]
        .codec_metadata
        .as_ref()
        .expect("codec_metadata present");
    assert!(codec_meta.has_bom);
    // Strip "text/" prefix
    assert_eq!(codec_meta.codec_name, "utf-8-bom");
}

// =========================================================================
// Codec test helpers (mirrors input_tests.rs make_codec_session pattern)
// =========================================================================

use {
    reovim_driver_codec::{
        CodecError, CodecMetadata as CMetadata, ContentCodec, ContentCodecFactory,
        ContentCodecFactoryStore, ContentType, DecodeResult,
    },
    reovim_kernel::api::v1::{BufferId as KBufferId, ByteEdit},
    std::sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
struct BufTestCodec {
    calls: std::sync::Arc<AtomicUsize>,
}

impl ContentCodec for BufTestCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CMetadata::new(ContentType::new("text/buf-test")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        _bytes: &dyn reovim_driver_vfs::ByteSource,
        _edit: &reovim_driver_codec::DecodedEdit,
    ) -> Result<Option<ByteEdit>, reovim_driver_codec::TranslateEditError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(ByteEdit::insert(0, b"x")))
    }
}

#[derive(Clone)]
struct BufTestCodecFactory {
    content_type: &'static str,
    calls: std::sync::Arc<AtomicUsize>,
}

impl ContentCodecFactory for BufTestCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<std::sync::Arc<dyn ContentCodec>> {
        if content_type.as_str() == self.content_type {
            Some(std::sync::Arc::new(BufTestCodec {
                calls: std::sync::Arc::clone(&self.calls),
            }))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![self.content_type]
    }

    fn name(&self) -> &'static str {
        "buf-test"
    }
}

/// Build a session with a `ContentCodecFactoryStore` registered in
/// kernel services.  Returns (registry, session, `call_counter`).
fn make_codec_session_for_buffer(
    content_type: &'static str,
) -> (Arc<SessionRegistry>, Arc<Session>, std::sync::Arc<AtomicUsize>) {
    use reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry};

    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let services = Arc::new(ServiceRegistry::new());

    let store = ContentCodecFactoryStore::new();
    store.add_factory(std::sync::Arc::new(BufTestCodecFactory {
        content_type,
        calls: std::sync::Arc::clone(&calls),
    }));
    services.register(std::sync::Arc::new(store));
    services.register(std::sync::Arc::new(reovim_driver_buffer::TextBufferRegistry::new()));

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_driver_buffer::TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        services,
    );
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));
    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    (registry, session, calls)
}

/// Register codec metadata and canonical bytes for a buffer.
///
/// Also sets the active view so that `unmount_codec` can find the buffer when
/// scanning `active_view` keys.
fn register_codec_buffer(session: &Arc<Session>, buffer_id: KBufferId, content_type: &str) {
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::{CodecSessionState, ContentCodecFactoryStore};

        let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
        codec_state.insert(buffer_id, CMetadata::new(ContentType::new(content_type)));
        // Set active_view so unmount_codec can find the buffer by scanning
        // the active_view map.
        codec_state.set_active_view(buffer_id, "default".to_string());
        let codec = state
            .app
            .kernel
            .services
            .get::<ContentCodecFactoryStore>()
            .and_then(|store| store.find(&ContentType::new(content_type)));
        if let Some(codec) = codec {
            codec_state.set_source_with_codec(buffer_id, b"hello".to_vec(), codec);
        } else {
            codec_state.set_source(buffer_id, b"hello".to_vec());
        }
    });
}

// =========================================================================
// get_codec_views: with factory store — views list building
// =========================================================================

#[tokio::test]
async fn test_get_codec_views_with_factory_and_codec_state() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::CodecSessionState;
        let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
        codec_state.set_active_view(buffer_id, "default".to_string());
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetCodecViewsRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
    });
    let response = service.get_codec_views(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffer_id, buffer_id.as_usize() as u64);
    assert_eq!(resp.active_view, "default");
    // Factory found, codec has default view.
    assert_eq!(resp.views.len(), 1);
    assert_eq!(resp.views[0].name, "default");
    assert_eq!(resp.views[0].display, "Default");
}

// =========================================================================
// switch_codec_view: success path
// =========================================================================

#[tokio::test]
async fn test_switch_codec_view_success() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        view_name: "default".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
    assert!(resp.error.is_none());
}

#[tokio::test]
async fn test_switch_codec_view_view_not_available() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        view_name: "nonexistent_view".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    // ViewNotAvailable → ok: false with error message (not a gRPC error).
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok);
    assert!(resp.error.is_some());
}

// =========================================================================
// mount_codec: success and error paths
// =========================================================================

#[tokio::test]
async fn test_mount_codec_success() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(MountCodecRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        content_type: "text/buf-test".to_string(),
        view_name: Some("default".to_string()),
        mount_mode: 0, // Summary
    });
    let response = service.mount_codec(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
    assert!(resp.error.is_none());
    assert!(resp.mount_id.is_some());
}

#[tokio::test]
async fn test_mount_codec_structural_mode() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(MountCodecRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        content_type: "text/buf-test".to_string(),
        view_name: Some("default".to_string()),
        mount_mode: 1, // Structural
    });
    let response = service.mount_codec(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

#[tokio::test]
async fn test_mount_codec_no_codec_factory_store() {
    // Session without factory store registered.
    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    // Register codec state but no factory store.
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::{CodecSessionState, ContentType};
        let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
        codec_state.insert(buffer_id, CMetadata::new(ContentType::new("text/buf-test")));
        codec_state.set_source(buffer_id, b"data".to_vec());
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(MountCodecRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        content_type: "text/buf-test".to_string(),
        view_name: None,
        mount_mode: 0,
    });
    let response = service.mount_codec(request).await;

    // No factory store → NotFound.
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_mount_codec_no_codec_state() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;
    // Don't register codec state.

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(MountCodecRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        content_type: "text/buf-test".to_string(),
        view_name: None,
        mount_mode: 0,
    });
    let response = service.mount_codec(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_mount_codec_no_codec_for_content_type() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(MountCodecRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        content_type: "text/unknown-type".to_string(), // Not registered.
        view_name: None,
        mount_mode: 0,
    });
    let response = service.mount_codec(request).await;

    // No codec for that content type → NotFound.
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_mount_codec_default_view_name_when_none() {
    // When view_name is None, should default to "default".
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(MountCodecRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        content_type: "text/buf-test".to_string(),
        view_name: None, // Should default to "default".
        mount_mode: 0,
    });
    let response = service.mount_codec(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

// =========================================================================
// umount_codec: success and error paths
// =========================================================================

#[tokio::test]
async fn test_umount_codec_success() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    // First mount it to get a mount_id.
    let service = BufferServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let mount_response = service
        .mount_codec(Request::new(MountCodecRequest {
            buffer_id: Some(buffer_id.as_usize() as u64),
            content_type: "text/buf-test".to_string(),
            view_name: Some("default".to_string()),
            mount_mode: 0,
        }))
        .await
        .expect("mount_codec ok")
        .into_inner();

    let mount_id = mount_response.mount_id.expect("mount_id present");
    assert!(mount_id > 0);

    // Now unmount.
    let request = Request::new(UmountCodecRequest { mount_id });
    let response = service.umount_codec(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
    assert!(resp.error.is_none());
}

#[tokio::test]
async fn test_umount_codec_zero_mount_id() {
    let (registry, _session) = test_registry_with_buffer_manager();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(UmountCodecRequest { mount_id: 0 });
    let response = service.umount_codec(request).await;

    // mount_id=0 is invalid.
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_umount_codec_not_found() {
    let (registry, session) = test_registry_with_buffer_manager();

    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::CodecSessionState;
        state.app.extensions.get_or_insert::<CodecSessionState>();
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    // Non-existent mount_id.
    let request = Request::new(UmountCodecRequest { mount_id: 9999 });
    let response = service.umount_codec(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_umount_codec_no_codec_state() {
    // No codec state at all.
    let (registry, _session) = test_registry_with_buffer_manager();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(UmountCodecRequest { mount_id: 1 });
    let response = service.umount_codec(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

// =========================================================================
// list_mounts: success paths
// =========================================================================

#[tokio::test]
async fn test_list_mounts_empty() {
    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    // Register codec state but no mounts.
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::{CodecSessionState, ContentType};
        let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
        codec_state.insert(buffer_id, CMetadata::new(ContentType::new("text/buf-test")));
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(ListMountsRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
    });
    let response = service.list_mounts(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.mounts.is_empty());
}

#[tokio::test]
async fn test_list_mounts_with_active_mount() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    register_codec_buffer(&session, buffer_id, "text/buf-test");

    let service = BufferServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    // Mount it first.
    #[allow(clippy::cast_possible_truncation)]
    let _ = service
        .mount_codec(Request::new(MountCodecRequest {
            buffer_id: Some(buffer_id.as_usize() as u64),
            content_type: "text/buf-test".to_string(),
            view_name: Some("default".to_string()),
            mount_mode: 0,
        }))
        .await
        .expect("mount ok");

    // List mounts.
    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(ListMountsRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
    });
    let response = service.list_mounts(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffer_id, buffer_id.as_usize() as u64);
    // At least one mount present.
    assert!(!resp.mounts.is_empty());
    let mount = &resp.mounts[0];
    assert_eq!(mount.view_name, "default");
    // Mode 0 = Summary.
    assert_eq!(mount.mode, 0);
}

#[tokio::test]
async fn test_list_mounts_structural_mode_in_proto() {
    // Use a fresh session without pre-existing mounts so the Structural mount
    // is the only (first) mount and we can assert mode=1 on it.
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    // Set source bytes WITHOUT a pre-mounted codec so mount_codec creates the
    // first (and only) mount in Structural mode.
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::{CodecSessionState, ContentType};
        let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
        codec_state.insert(buffer_id, CMetadata::new(ContentType::new("text/buf-test")));
        codec_state.set_active_view(buffer_id, "default".to_string());
        // set_source (no codec) creates the inode without any mount.
        codec_state.set_source(buffer_id, b"hello".to_vec());
    });

    let service = BufferServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    // Mount in Structural mode (mode=1) — this is the first mount.
    #[allow(clippy::cast_possible_truncation)]
    let _ = service
        .mount_codec(Request::new(MountCodecRequest {
            buffer_id: Some(buffer_id.as_usize() as u64),
            content_type: "text/buf-test".to_string(),
            view_name: Some("default".to_string()),
            mount_mode: 1, // Structural
        }))
        .await
        .expect("mount ok");

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(ListMountsRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
    });
    let response = service.list_mounts(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // The single mount should have mode=1 (Structural).
    assert_eq!(resp.mounts.len(), 1);
    assert_eq!(resp.mounts[0].mode, 1, "Expected Structural (1) mode in proto");
}

#[tokio::test]
async fn test_list_mounts_no_codec_state() {
    let (registry, session) = test_registry_with_buffer_manager();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    // No codec state → empty mounts list (unwrap_or_default).
    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(ListMountsRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
    });
    let response = service.list_mounts(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.mounts.is_empty());
}

// =========================================================================
// list_available_codecs
// =========================================================================

#[tokio::test]
async fn test_list_available_codecs_empty() {
    // No factory store → empty codec list.
    let (registry, _session) = test_registry_with_buffer_manager();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListAvailableCodecsRequest {});
    let response = service.list_available_codecs(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.codecs.is_empty());
}

#[tokio::test]
async fn test_list_available_codecs_with_factory() {
    let (registry, _session, _calls) = make_codec_session_for_buffer("text/buf-test");
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListAvailableCodecsRequest {});
    let response = service.list_available_codecs(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // One factory registered: "buf-test" for "text/buf-test".
    assert_eq!(resp.codecs.len(), 1);
    assert_eq!(resp.codecs[0].name, "buf-test");
    assert!(
        resp.codecs[0]
            .content_types
            .contains(&"text/buf-test".to_string())
    );
}

// =========================================================================
// get_codec_views: client_active path
// =========================================================================

#[tokio::test]
async fn test_get_codec_views_uses_client_active_buffer() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let (_buf1, buf2) = session
        .with_state_mut(|state| {
            let b1 = state.create_buffer("first");
            let b2 = state.create_buffer("second");
            (b1, b2)
        })
        .await;

    register_codec_buffer(&session, buf2, "text/buf-test");
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::CodecSessionState;
        let codec_state = state.app.extensions.get_or_insert::<CodecSessionState>();
        codec_state.set_active_view(buf2, "default".to_string());
    });

    let client_id = ClientId::new(9);
    session.add_client(client_id);
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buf2);
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(GetCodecViewsRequest { buffer_id: None });
    request.extensions_mut().insert(client_id);
    let response = service.get_codec_views(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffer_id, buf2.as_usize() as u64);
}

// =========================================================================
// mount_codec: client_active path
// =========================================================================

#[tokio::test]
async fn test_mount_codec_uses_client_active_buffer() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let (_buf1, buf2) = session
        .with_state_mut(|state| {
            let b1 = state.create_buffer("first");
            let b2 = state.create_buffer("second");
            (b1, b2)
        })
        .await;

    register_codec_buffer(&session, buf2, "text/buf-test");

    let client_id = ClientId::new(10);
    session.add_client(client_id);
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buf2);
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let mut request = Request::new(MountCodecRequest {
        buffer_id: None, // Resolved via client_active.
        content_type: "text/buf-test".to_string(),
        view_name: Some("default".to_string()),
        mount_mode: 0,
    });
    request.extensions_mut().insert(client_id);
    let response = service.mount_codec(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

// =========================================================================
// list_mounts: client_active path
// =========================================================================

#[tokio::test]
async fn test_list_mounts_uses_client_active_buffer() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let (_buf1, buf2) = session
        .with_state_mut(|state| {
            let b1 = state.create_buffer("first");
            let b2 = state.create_buffer("second");
            (b1, b2)
        })
        .await;

    register_codec_buffer(&session, buf2, "text/buf-test");

    let client_id = ClientId::new(11);
    session.add_client(client_id);
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buf2);
    });

    // Mount buf2 first (using explicit buffer_id).
    let svc = BufferServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));
    #[allow(clippy::cast_possible_truncation)]
    let _ = svc
        .mount_codec(Request::new(MountCodecRequest {
            buffer_id: Some(buf2.as_usize() as u64),
            content_type: "text/buf-test".to_string(),
            view_name: Some("default".to_string()),
            mount_mode: 0,
        }))
        .await
        .expect("mount ok");

    // Now list via client_active (no explicit buffer_id).
    let mut request = Request::new(ListMountsRequest { buffer_id: None });
    request.extensions_mut().insert(client_id);
    let response = svc.list_mounts(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffer_id, buf2.as_usize() as u64);
}

// =========================================================================
// switch_codec_view: client_active path (L365-369)
// =========================================================================

/// When a `ClientId` is in the request extensions and the client has
/// `active_buffer` set, `switch_codec_view` should use that buffer (no
/// explicit `buffer_id` in the request).
#[tokio::test]
async fn test_switch_codec_view_uses_client_active_buffer() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let (_buf1, buf2) = session
        .with_state_mut(|state| {
            let b1 = state.create_buffer("first");
            let b2 = state.create_buffer("second");
            (b1, b2)
        })
        .await;

    register_codec_buffer(&session, buf2, "text/buf-test");

    let client_id = ClientId::new(11);
    session.add_client(client_id);
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buf2);
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    // No buffer_id — relies on client_active resolution.
    let mut request = Request::new(SwitchCodecViewRequest {
        buffer_id: None,
        view_name: "default".to_string(),
    });
    request.extensions_mut().insert(client_id);
    let response = service.switch_codec_view(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

// =========================================================================
// switch_codec_view: SwitchViewError::NoMetadata (L399)
// =========================================================================

/// When `CodecSessionState` extension exists but has no entry for the
/// buffer, `switch_view` returns `NoMetadata`.
#[tokio::test]
async fn test_switch_codec_view_no_metadata_for_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            let bid = state.create_buffer("content");
            // Insert an empty CodecSessionState (no entry for `bid`).
            state
                .app
                .extensions
                .get_or_insert::<reovim_driver_codec::CodecSessionState>();
            bid
        })
        .await;

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: None,
        view_name: "default".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    // CodecSessionState exists (preflight passes), but no metadata → NotFound.
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

// =========================================================================
// switch_codec_view: SwitchViewError::DecodeFailed (L415-418)
// =========================================================================

/// A codec whose `decode_view` always returns an error.
#[derive(Clone)]
struct FailingCodec;

impl ContentCodec for FailingCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CMetadata::new(ContentType::new("text/failing")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        _bytes: &dyn reovim_driver_vfs::ByteSource,
        _edit: &reovim_driver_codec::DecodedEdit,
    ) -> Result<Option<ByteEdit>, reovim_driver_codec::TranslateEditError> {
        Err(reovim_driver_codec::TranslateEditError::ReadOnly)
    }

    fn views(&self) -> &[reovim_driver_codec::CodecView] {
        &[reovim_driver_codec::CodecView::DEFAULT]
    }

    fn decode_view(&self, _raw: &[u8], _view: &str) -> Result<DecodeResult, CodecError> {
        Err(CodecError::Other("simulated decode failure".into()))
    }
}

#[derive(Clone)]
struct FailingCodecFactory;

impl ContentCodecFactory for FailingCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<std::sync::Arc<dyn ContentCodec>> {
        if content_type.as_str() == "text/failing" {
            Some(std::sync::Arc::new(FailingCodec))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec!["text/failing"]
    }

    fn name(&self) -> &'static str {
        "failing"
    }
}

/// Build a session with a factory that produces a `FailingCodec`.
fn make_failing_codec_session() -> (Arc<SessionRegistry>, Arc<Session>) {
    use reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry};

    let services = Arc::new(ServiceRegistry::new());

    let store = ContentCodecFactoryStore::new();
    store.add_factory(std::sync::Arc::new(FailingCodecFactory));
    services.register(std::sync::Arc::new(store));
    services.register(std::sync::Arc::new(reovim_driver_buffer::TextBufferRegistry::new()));

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_driver_buffer::TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        services,
    );
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));
    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    (registry, session)
}

/// `switch_codec_view` should return `Internal` when the codec's
/// `decode_view` returns an error.
#[tokio::test]
async fn test_switch_codec_view_decode_failed() {
    let (registry, session) = make_failing_codec_session();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("raw content"))
        .await;

    // Register codec metadata with canonical bytes so switch_view proceeds
    // past NoMetadata / NoCanonicalBytes / NoCodec.
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::{ContentCodecFactoryStore, ContentType};

        let codec_state = state
            .app
            .extensions
            .get_or_insert::<reovim_driver_codec::CodecSessionState>();
        codec_state.insert(buffer_id, CMetadata::new(ContentType::new("text/failing")));
        let codec = state
            .app
            .kernel
            .services
            .get::<ContentCodecFactoryStore>()
            .and_then(|store| store.find(&ContentType::new("text/failing")));
        if let Some(codec) = codec {
            codec_state.set_source_with_codec(buffer_id, b"bytes".to_vec(), codec);
        }
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(SwitchCodecViewRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        view_name: "default".to_string(),
    });
    let response = service.switch_codec_view(request).await;

    // decode_view failure → Internal gRPC error.
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Internal);
}

// =========================================================================
// mount_codec: MountCodecError::NoCanonicalBytes (L507)
// =========================================================================

/// `mount_codec` returns `FailedPrecondition` when `CodecSessionState` has
/// metadata for the buffer but no canonical inode bytes (no `set_source`
/// call).
#[tokio::test]
async fn test_mount_codec_no_canonical_bytes() {
    let (registry, session, _calls) = make_codec_session_for_buffer("text/buf-test");

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("content"))
        .await;

    // Register codec metadata but do NOT set canonical bytes.
    session.with_state_mut_sync(|state| {
        let codec_state = state
            .app
            .extensions
            .get_or_insert::<reovim_driver_codec::CodecSessionState>();
        codec_state.insert(buffer_id, CMetadata::new(ContentType::new("text/buf-test")));
        // Deliberately no set_source / set_source_with_codec call.
    });

    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(MountCodecRequest {
        buffer_id: Some(buffer_id.as_usize() as u64),
        content_type: "text/buf-test".to_string(),
        view_name: Some("default".to_string()),
        mount_mode: 0,
    });
    let response = service.mount_codec(request).await;

    // No inode → NoCanonicalBytes → FailedPrecondition.
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::FailedPrecondition);
}
