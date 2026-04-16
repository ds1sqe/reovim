use super::*;

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

// =========================================================================
// BufferServiceImpl construction
// =========================================================================

#[test]
fn test_buffer_service_new() {
    let registry = Arc::new(SessionRegistry::new());
    let service = BufferServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));
    let _ = service;
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

// =========================================================================
// List buffers (v3 proto: BufferInfo without line_count/codec_metadata)
// =========================================================================

#[tokio::test]
async fn test_list_buffers_empty() {
    let registry = test_registry();
    let service = BufferServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(ListBuffersRequest {});
    let response = service.list(request).await;

    // Stub returns Unimplemented until wired to kernel
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
}

// =========================================================================
// OpenFile (stub)
// =========================================================================

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

// =========================================================================
// WriteFile (stub)
// =========================================================================

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

// =========================================================================
// SetContent (stub)
// =========================================================================

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

// GetRawContent, GetLineCount, GetAnnotations: DELETED (#753 D-chain)
// All codec RPCs (mount, unmount, switch_view, get_codec_views, list_mounts,
// list_available_codecs): DELETED (#753 D-chain)
// These RPCs no longer exist in proto v3.
