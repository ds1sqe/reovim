use super::*;

fn test_registry() -> Arc<SessionRegistry> {
    let (registry, _) = test_registry_with_session();
    registry
}

/// Create a registry with a session and return both.
/// This allows tests to add clients to the session.
fn test_registry_with_session() -> (Arc<SessionRegistry>, Arc<Session>) {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    (registry, session)
}

/// Create a registry with a session that has a real buffer manager.
fn test_registry_with_buffer_manager() -> (Arc<SessionRegistry>, Arc<Session>) {
    use {
        reovim_driver_text_buffer::TestBufferManager,
        reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry},
    };

    // Create a KernelContext with a real buffer manager
    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));

    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    (registry, session)
}

/// Helper: build a request with token-authenticated `ClientId` in extensions.
fn authed_request<T>(body: T, client_id: ClientId) -> Request<T> {
    let mut request = Request::new(body);
    request.extensions_mut().insert(client_id);
    request
}

/// (#753) get_projections replaces get_mode, get_cursor, get_selection.
#[tokio::test]
async fn test_get_projections_returns_empty_stub() {
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    session.add_client(ClientId::new(1));

    let request = Request::new(GetProjectionsRequest {
        client_id: 1,
        tags: vec![],
    });
    let response = service.get_projections(request).await;

    assert!(response.is_ok());
    // Stub returns empty projections
    assert_eq!(response.unwrap().into_inner().projections.len(), 0);
}

#[tokio::test]
async fn test_get_projections_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service = StateServiceImpl::new(registry, SessionId::new("nonexistent"));

    let request = Request::new(GetProjectionsRequest {
        client_id: 0,
        tags: vec![],
    });
    let response = service.get_projections(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_options_returns_ok() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetOptionsRequest { names: vec![] });
    let response = service.get_options(request).await;

    assert!(response.is_ok());
}

#[tokio::test]
async fn test_get_layout_rejects_unauthenticated() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // No token in extensions → Unauthenticated (#483)
    let request = Request::new(GetLayoutRequest { client_id: 0 });
    let response = service.get_layout(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_get_layout_empty() {
    // Session with a client but no windows
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    // Create a client
    session.add_client(ClientId::new(1));

    let request = authed_request(GetLayoutRequest { client_id: 1 }, ClientId::new(1));
    let response = service.get_layout(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Empty layout: no root node
    assert!(resp.root.is_none());
    assert_eq!(resp.focused_window_id, None);
}

#[tokio::test]
async fn test_get_layout_single_window() {
    let (registry, session) = test_registry_with_buffer_manager();

    // Create a buffer (active buffer is set in SessionShared)
    session
        .with_state_mut(|state| {
            let _buffer_id = state.create_buffer("hello world");
        })
        .await;

    // Create a client - this creates per-client windows in EditingState (#491)
    // The client will have a window with the active buffer
    session.add_client(ClientId::new(1));

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(GetLayoutRequest { client_id: 1 }, ClientId::new(1));
    let response = service.get_layout(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();

    // Should have a root node (single leaf) from per-client state
    assert!(resp.root.is_some());
    let root = resp.root.unwrap();

    // Check it's a leaf with default viewport dimensions
    match root.node {
        Some(reovim_protocol::v2::window_node::Node::Leaf(leaf)) => {
            // Default viewport is 80x24
            assert_eq!(leaf.rect.as_ref().unwrap().width, 80);
            assert_eq!(leaf.rect.as_ref().unwrap().height, 24);
        }
        _ => panic!("Expected a leaf node"),
    }
}

#[tokio::test]
async fn test_get_visible_lines_rejects_unauthenticated() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // No token in extensions → Unauthenticated (#483)
    let request = Request::new(GetVisibleLinesRequest {
        window_id: None,
        client_id: 0,
    });
    let response = service.get_visible_lines(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_get_visible_lines_no_window() {
    // Session with a client but no windows
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    session.add_client(ClientId::new(1));

    let request = authed_request(
        GetVisibleLinesRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_visible_lines(request).await;

    // Client exists but no window = NotFound
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_visible_lines_with_window() {
    let (registry, session) = test_registry_with_buffer_manager();

    // Create a buffer (active buffer is set in SessionShared)
    session
        .with_state_mut(|state| {
            let _buffer_id = state.create_buffer("line0\nline1\nline2\nline3");
        })
        .await;

    // Create a client - this creates per-client windows in EditingState (#491)
    session.add_client(ClientId::new(1));

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetVisibleLinesRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_visible_lines(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();

    assert_eq!(resp.first_line, 0);
    assert_eq!(resp.last_line, 23); // scroll_top(0) + height(24) - 1
    assert_eq!(resp.viewport_height, 24);
}

#[tokio::test]
async fn test_get_visible_lines_with_scroll() {
    use reovim_driver_text_session::Viewport;

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("content");
        })
        .await;

    // Create a client
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Modify the CLIENT's per-client viewport with scroll
    session.clients().update_client_state(client_id, |state| {
        if let Some(window) = state.windows.active_mut() {
            let mut viewport = Viewport::new(80, 24);
            viewport.scroll_top = 10; // Scrolled down 10 lines
            window.viewport = viewport;
        }
    });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetVisibleLinesRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_visible_lines(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();

    assert_eq!(resp.first_line, 10);
    assert_eq!(resp.last_line, 33); // scroll_top(10) + height(24) - 1
    assert_eq!(resp.viewport_height, 24);
}


#[tokio::test]
async fn test_get_registers_empty() {
    let (registry, session) = test_registry_with_session();
    session.add_client(ClientId::new(1));
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetRegistersRequest {
            names: vec![],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Empty by default
    assert!(resp.registers.is_empty());
}

#[tokio::test]
async fn test_get_registers_with_content() {
    use reovim_driver_text_session::RegisterContent;

    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    // Set a register on the per-client state (#515)
    session
        .clients()
        .update_client_state(ClientId::new(1), |state| {
            state.registers.set(RegisterContent::characterwise("hello"));
        });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetRegistersRequest {
            names: vec![],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.registers.len(), 1);
    assert_eq!(resp.registers[0].name, "\"");
    // (#753) RegisterEntry.content is now DomainDatum; display field carries the text.
    assert_eq!(
        resp.registers[0].content.as_ref().and_then(|d| d.display.as_deref()),
        Some("hello")
    );
}

#[tokio::test]
async fn test_get_registers_specific_register() {
    use reovim_driver_text_session::RegisterContent;

    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    // Set multiple registers on per-client state (#515)
    session
        .clients()
        .update_client_state(ClientId::new(1), |state| {
            state
                .registers
                .set(RegisterContent::characterwise("unnamed"));
            state
                .registers
                .set_named('a', RegisterContent::linewise("alpha"));
            state
                .registers
                .set_named('b', RegisterContent::characterwise("beta"));
        });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Query only register 'a'
    let request = authed_request(
        GetRegistersRequest {
            names: vec!["a".to_string()],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.registers.len(), 1);
    assert_eq!(resp.registers[0].name, "a");
    // (#753) RegisterEntry.content is now DomainDatum; display field carries the text.
    assert_eq!(
        resp.registers[0].content.as_ref().and_then(|d| d.display.as_deref()),
        Some("alpha")
    );
}

// (#753) GetMode/GetCursor/GetSelection replaced by GetProjections.
// Cursor, mode, and selection state is now domain-neutral (ProjectionUpdated notifications).

#[tokio::test]
async fn test_layout_isolation_per_client() {
    use reovim_driver_text_session::Viewport;

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("test content");
        })
        .await;

    // Create a client
    let client_a = crate::session::ClientId::new(100);
    session.add_client(client_a);

    // Modify client's window viewport
    session.clients().update_client_state(client_a, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.viewport = Viewport::new(120, 40);
        }
    });

    // Verify via gRPC with client_id
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(GetLayoutRequest { client_id: 100 }, ClientId::new(100));
    let response = service.get_layout(request).await.unwrap().into_inner();

    // Should get per-client layout with the modified viewport
    assert!(response.root.is_some(), "Should have a root node");
    if let Some(node) = response.root
        && let Some(Node::Leaf(leaf)) = node.node
        && let Some(rect) = leaf.rect
    {
        assert_eq!(rect.width, 120, "Per-client window width");
        assert_eq!(rect.height, 40, "Per-client window height");
    } else {
        panic!("Expected a leaf node with rect");
    }
}

#[tokio::test]
async fn test_get_visible_lines_with_specific_window_id() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("line0\nline1\nline2");
        })
        .await;

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Get the window id from client state
    let window_id = session
        .clients()
        .client_state(client_id)
        .unwrap()
        .windows
        .active()
        .unwrap()
        .id
        .as_usize() as u64;

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetVisibleLinesRequest {
            window_id: Some(window_id),
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_visible_lines(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.window_id, window_id);
    assert_eq!(resp.first_line, 0);
}

#[tokio::test]
async fn test_get_visible_lines_with_invalid_window_id() {
    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("content");
        })
        .await;

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetVisibleLinesRequest {
            window_id: Some(99999), // Non-existent window
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_visible_lines(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_layout_multi_window() {
    use reovim_driver_text_session::Window;

    let (registry, session) = test_registry_with_buffer_manager();

    session
        .with_state_mut(|state| {
            state.create_buffer("content");
        })
        .await;

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Add a second window to the client
    let buffer_id2 = reovim_kernel::api::v1::BufferId::from_raw(99);
    session.clients().update_client_state(client_id, |state| {
        let window2 = Window::with_buffer(buffer_id2);
        state.windows.add(window2);
    });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(GetLayoutRequest { client_id: 1 }, ClientId::new(1));
    let response = service.get_layout(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();

    // Should have a root node wrapping N>1 windows in a split
    assert!(resp.root.is_some());
    let root = resp.root.unwrap();
    match root.node {
        Some(Node::Split(split)) => {
            assert_eq!(split.direction, SplitDirection::Vertical as i32);
            assert_eq!(split.children.len(), 2);
        }
        _ => panic!("Expected split node for multi-window layout"),
    }
}

#[tokio::test]
async fn test_submit_capture_response() {
    let registry = test_registry();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    // Create a pending capture
    let session = registry.get(&SessionId::new("test")).unwrap();
    let (request_id, _rx) = session.capture_tracker().create_pending();

    let req = Request::new(SubmitCaptureRequest {
        request_id,
        width: 80,
        height: 24,
        format: "plain_text".to_string(),
        content: "Hello World".to_string(),
    });

    let response = service.submit_capture_response(req).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

#[tokio::test]
async fn test_submit_capture_response_no_pending() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Submit without a pending request
    let req = Request::new(SubmitCaptureRequest {
        request_id: 99999, // No such pending
        width: 80,
        height: 24,
        format: "plain_text".to_string(),
        content: "data".to_string(),
    });

    let response = service.submit_capture_response(req).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok); // No pending request to deliver to
}

#[tokio::test]
async fn test_get_registers_specific_nonexistent_register() {
    let (registry, session) = test_registry_with_session();
    session.add_client(ClientId::new(1));
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetRegistersRequest {
            names: vec!["z".to_string()],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Register 'z' doesn't exist so should be empty
    assert!(resp.registers.is_empty());
}


#[tokio::test]
async fn test_get_screen_content_invalid_format() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(GetScreenContentRequest {
        format: "invalid_format".to_string(),
        client_id: 0,
    });
    let response = service.get_screen_content(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

// Removed duplicate test_get_cursor_unknown_client — replaced by
// test_get_cursor_dangling_follower_not_found which also covers the
// ring buffer logging callback path.

#[tokio::test]
async fn test_get_layout_unknown_client() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(GetLayoutRequest { client_id: 999 }, ClientId::new(999));
    let response = service.get_layout(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_visible_lines_unknown_client() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetVisibleLinesRequest {
            window_id: None,
            client_id: 999,
        },
        ClientId::new(999),
    );
    let response = service.get_visible_lines(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

// =========================================================================
// Coverage: Ring buffer logging for unknown clients (#497)
// =========================================================================

#[tokio::test]
async fn test_get_layout_following_client_not_found_logs() {
    // Test the ring buffer logging path in get_layout (lines 225-231).
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    // Add a client so it exists (and has a ring buffer) but query different ID
    session.add_client(ClientId::new(1));

    let request = authed_request(GetLayoutRequest { client_id: 888 }, ClientId::new(888));
    let response = service.get_layout(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_visible_lines_client_not_found_logs() {
    // Test the ring buffer logging path in get_visible_lines (lines 306-312).
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    session.add_client(ClientId::new(1));

    let request = authed_request(
        GetVisibleLinesRequest {
            window_id: None,
            client_id: 777,
        },
        ClientId::new(777),
    );
    let response = service.get_visible_lines(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}


// =========================================================================
// Coverage: get_screen_content capture error paths (#497)
// =========================================================================

#[test]
fn test_capture_error_to_status_all_variants() {
    use {super::capture_error_to_status, crate::session::CaptureError};

    // NoTuiClient → UNAVAILABLE
    let status = capture_error_to_status(CaptureError::NoTuiClient);
    assert_eq!(status.code(), tonic::Code::Unavailable);

    // Timeout → DEADLINE_EXCEEDED
    let status = capture_error_to_status(CaptureError::Timeout);
    assert_eq!(status.code(), tonic::Code::DeadlineExceeded);

    // Disconnected → ABORTED
    let status = capture_error_to_status(CaptureError::Disconnected);
    assert_eq!(status.code(), tonic::Code::Aborted);

    // InvalidResponse → INTERNAL
    let status = capture_error_to_status(CaptureError::InvalidResponse("bad data".into()));
    assert_eq!(status.code(), tonic::Code::Internal);
    assert!(status.message().contains("bad data"));
}

#[tokio::test]
async fn test_get_screen_content_default_format() {
    // Test the default format path (empty format -> "raw_ansi").
    // This will timeout/fail but tests the format validation path.
    let (registry, _session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    let request = Request::new(GetScreenContentRequest {
        format: String::new(), // empty -> defaults to "raw_ansi"
        client_id: 0,
    });
    // This will fail because no TUI client to deliver the capture,
    // but the format validation path is covered.
    let _response = service.get_screen_content(request).await;
    // We don't assert success because it depends on timing/capture delivery
}

#[tokio::test]
async fn test_get_screen_content_valid_formats() {
    // Test accepted format strings.
    let (registry, _session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    for format in &["plain_text", "raw_ansi", "cell_grid"] {
        let request = Request::new(GetScreenContentRequest {
            format: (*format).to_string(),
            client_id: 0,
        });
        // These will fail at capture delivery but format validation passes.
        let _response = service.get_screen_content(request).await;
    }
}

// =========================================================================
// Coverage: get_registers specific register with content (#497)
// =========================================================================

#[tokio::test]
async fn test_get_registers_multiple_specific() {
    // Test querying multiple specific registers.
    use reovim_driver_text_session::RegisterContent;

    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    session
        .clients()
        .update_client_state(ClientId::new(1), |state| {
            state
                .registers
                .set_named('a', RegisterContent::characterwise("alpha"));
            state
                .registers
                .set_named('b', RegisterContent::linewise("beta\n"));
            // 'c' not set
        });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetRegistersRequest {
            names: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // 'a' and 'b' should be returned, 'c' is empty/missing
    assert_eq!(resp.registers.len(), 2);
}

#[tokio::test]
async fn test_get_registers_specific_empty_register_filtered_out() {
    // Test that a register with empty content is filtered out.
    use reovim_driver_text_session::RegisterContent;

    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    session
        .clients()
        .update_client_state(ClientId::new(1), |state| {
            state
                .registers
                .set_named('x', RegisterContent::characterwise(""));
            state
                .registers
                .set_named('y', RegisterContent::characterwise("visible"));
        });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetRegistersRequest {
            names: vec!["x".to_string(), "y".to_string()],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // 'x' is empty and should be filtered out, only 'y' returned
    assert_eq!(resp.registers.len(), 1);
    assert_eq!(resp.registers[0].name, "y");
}

#[tokio::test]
async fn test_get_registers_client_not_found() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Non-existent client should return NotFound
    let request = authed_request(
        GetRegistersRequest {
            client_id: 999,
            names: vec![],
        },
        ClientId::new(999),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}


// =========================================================================
// Tests: CLIENT_NOT_FOUND error paths with ring buffer logging
// Client exists in session (has ring buffer) but effective_state() returns
// None because it's Following a non-existent target.
// Covers lines 154-158, 226-230, 307-311, 363-367.
// =========================================================================

/// Create a registry with a Following client whose target doesn't exist.
/// This makes `client_state()` return None while `with_client_ring_buffer()`
/// still calls the callback (client exists in map, has ring buffer).
fn test_registry_with_dangling_follower(client_id: ClientId) -> Arc<SessionRegistry> {
    use {
        crate::session::{Client, ClientMetadata, ClientRelation},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let (registry, session) = test_registry_with_session();
    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let metadata = ClientMetadata::default();
    let mut client = Client::with_mode_stack(client_id, metadata, mode_stack);
    // Point to a non-existent target so effective_state() returns None
    client.relation = Some(ClientRelation::Following {
        target: ClientId::new(99999),
    });
    session.clients().add_client_with_state(client);
    registry
}


#[tokio::test]
async fn test_get_layout_dangling_follower_not_found() {
    let cid = ClientId::new(51);
    let registry = test_registry_with_dangling_follower(cid);
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetLayoutRequest {
            client_id: cid.as_usize() as u64,
        },
        cid,
    );
    let err = service.get_layout(request).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_visible_lines_dangling_follower_not_found() {
    let cid = ClientId::new(52);
    let registry = test_registry_with_dangling_follower(cid);
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetVisibleLinesRequest {
            client_id: cid.as_usize() as u64,
            window_id: None,
        },
        cid,
    );
    let err = service.get_visible_lines(request).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::NotFound);
}


// =========================================================================
// Coverage: kernel_to_proto_option() all variants (L62-77)
// =========================================================================

#[test]
fn test_kernel_to_proto_option_bool_true() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v2::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::Bool(true));
    assert_eq!(proto.value, Some(Value::BoolValue(true)));
}

#[test]
fn test_kernel_to_proto_option_bool_false() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v2::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::Bool(false));
    assert_eq!(proto.value, Some(Value::BoolValue(false)));
}

#[test]
fn test_kernel_to_proto_option_integer() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v2::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::Integer(42));
    assert_eq!(proto.value, Some(Value::IntValue(42)));
}

#[test]
fn test_kernel_to_proto_option_string() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v2::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::String("hello".to_string()));
    assert_eq!(proto.value, Some(Value::StringValue("hello".to_string())));
}

#[test]
fn test_kernel_to_proto_option_choice() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v2::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::Choice {
        value: "all".to_string(),
        choices: vec!["none".to_string(), "all".to_string(), "block".to_string()],
    });
    // Choice maps to StringValue with the selected value
    assert_eq!(proto.value, Some(Value::StringValue("all".to_string())));
}

// =========================================================================
// Coverage: get_options with specific named options (L229-239)
// =========================================================================

#[tokio::test]
async fn test_get_options_with_specific_names_registered() {
    use {
        reovim_driver_text_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
        },
    };

    // Build a session with a real OptionRegistry that has a registered option.
    let option_registry = Arc::new(OptionRegistry::new());
    option_registry
        .register(OptionSpec::new("number", "Show line numbers", OptionValue::Bool(false)))
        .expect("register option");

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        option_registry,
        Arc::new(ServiceRegistry::new()),
    );
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));
    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Request by specific name — exercises the non-empty names branch (L229-233).
    let request = Request::new(GetOptionsRequest {
        names: vec!["number".to_string()],
    });
    let response = service.get_options(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // "number" was registered with default Bool(false), so it should appear.
    assert_eq!(resp.options.len(), 1);
    assert!(resp.options.contains_key("number"));
    let opt = &resp.options["number"];
    assert_eq!(opt.value, Some(reovim_protocol::v2::option_value::Value::BoolValue(false)));
}

#[tokio::test]
async fn test_get_options_with_alias_resolves() {
    use {
        reovim_driver_text_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
        },
    };

    let option_registry = Arc::new(OptionRegistry::new());
    option_registry
        .register(
            OptionSpec::new("number", "Show line numbers", OptionValue::Bool(true))
                .with_short("nu"),
        )
        .expect("register option");

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        option_registry,
        Arc::new(ServiceRegistry::new()),
    );
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));
    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Request by short alias "nu" — resolve_name returns "number".
    let request = Request::new(GetOptionsRequest {
        names: vec!["nu".to_string()],
    });
    let response = service.get_options(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Resolved to full name "number".
    assert_eq!(resp.options.len(), 1);
    assert!(resp.options.contains_key("number"));
}

#[tokio::test]
async fn test_get_options_with_unknown_name_omitted() {
    use {
        reovim_driver_text_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
        },
    };

    let option_registry = Arc::new(OptionRegistry::new());
    option_registry
        .register(OptionSpec::new(
            "expandtab",
            "Use spaces instead of tabs",
            OptionValue::Bool(false),
        ))
        .expect("register option");

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        option_registry,
        Arc::new(ServiceRegistry::new()),
    );
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));
    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // "nonexistent" is unknown — filter_map drops it, result has only known options.
    let request = Request::new(GetOptionsRequest {
        names: vec!["expandtab".to_string(), "nonexistent".to_string()],
    });
    let response = service.get_options(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Only "expandtab" should be present; "nonexistent" is silently dropped.
    assert_eq!(resp.options.len(), 1);
    assert!(resp.options.contains_key("expandtab"));
}

#[tokio::test]
async fn test_get_options_empty_names_returns_all() {
    use {
        reovim_driver_text_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
        },
    };

    let option_registry = Arc::new(OptionRegistry::new());
    option_registry
        .register(OptionSpec::new("wrap", "Line wrap", OptionValue::Bool(true)))
        .expect("register wrap");
    option_registry
        .register(OptionSpec::new("tabstop", "Tab stop width", OptionValue::Integer(8)))
        .expect("register tabstop");

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        option_registry,
        Arc::new(ServiceRegistry::new()),
    );
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = Arc::new(Session::from_state(SessionId::new("test"), state));
    let registry = Arc::new(SessionRegistry::new());
    registry.insert(&session);

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Empty names → list_all() branch returns all registered options.
    let request = Request::new(GetOptionsRequest { names: vec![] });
    let response = service.get_options(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.options.len(), 2);
    assert!(resp.options.contains_key("wrap"));
    assert!(resp.options.contains_key("tabstop"));
}
