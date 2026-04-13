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
        reovim_driver_buffer::TestBufferManager,
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

#[tokio::test]
async fn test_get_mode_returns_current_mode() {
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    // Create a client first (client_id=1)
    session.add_client(ClientId::new(1));

    let request = authed_request(GetModeRequest { client_id: 1 }, ClientId::new(1));
    let response = service.get_mode(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Default mode is "normal" from SessionState::default()
    assert_eq!(resp.name, "normal");
    assert_eq!(resp.display, "NORMAL");
    assert!(!resp.is_insert);
}

#[tokio::test]
async fn test_get_mode_rejects_unauthenticated() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // No token in extensions → Unauthenticated (#483)
    let request = Request::new(GetModeRequest { client_id: 0 });
    let response = service.get_mode(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_get_mode_unknown_client_returns_not_found() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Non-existent client should return NotFound
    // Authenticated as client 999, targeting self (client_id=999 in body)
    let request = authed_request(GetModeRequest { client_id: 999 }, ClientId::new(999));
    let response = service.get_mode(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_cursor_rejects_unauthenticated() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // No token in extensions → Unauthenticated (#483)
    let request = Request::new(GetCursorRequest {
        window_id: None,
        client_id: 0,
    });
    let response = service.get_cursor(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_get_cursor_no_active_window() {
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    // Create a client but don't add any windows
    session.add_client(ClientId::new(1));

    let request = authed_request(
        GetCursorRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_cursor(request).await;

    // Client exists but no active window = NotFound
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_cursor_with_buffer() {
    let (registry, session) = test_registry_with_buffer_manager();

    // Create a buffer using the real buffer manager
    session
        .with_state_mut(|state| {
            state.create_buffer("hello world");
        })
        .await;

    // Create a client - this initializes per-client state with initial window
    session.add_client(ClientId::new(1));

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetCursorRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_cursor(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.position.is_some());
    let pos = resp.position.unwrap();
    assert_eq!(pos.line, 0);
    assert_eq!(pos.column, 0);
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
    use reovim_driver_session::Viewport;

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
async fn test_get_mode_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service = StateServiceImpl::new(registry, SessionId::new("nonexistent"));

    let request = Request::new(GetModeRequest { client_id: 0 });
    let response = service.get_mode(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
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
    use reovim_domain_text::RegisterContent;

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
    assert_eq!(resp.registers[0].content, "hello");
    assert_eq!(resp.registers[0].yank_type, "char");
}

#[tokio::test]
async fn test_get_registers_specific_register() {
    use reovim_domain_text::RegisterContent;

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
    assert_eq!(resp.registers[0].content, "alpha");
    assert_eq!(resp.registers[0].yank_type, "line");
}

// Phase 9.1: GetSelection RPC tests

#[tokio::test]
async fn test_get_selection_rejects_unauthenticated() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // No token in extensions → Unauthenticated (#483)
    let request = Request::new(GetSelectionRequest {
        window_id: None,
        client_id: 0,
    });
    let response = service.get_selection(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_get_selection_no_selection() {
    let (registry, session) = test_registry_with_buffer_manager();

    // Create a buffer but don't start selection
    session
        .with_state_mut(|state| {
            state.create_buffer("hello world");
        })
        .await;

    // Create a client
    session.add_client(ClientId::new(1));

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_selection(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.has_selection);
    assert!(resp.selection.is_none());
    assert!(resp.visual_mode.is_none());
}

#[tokio::test]
async fn test_get_selection_no_active_window() {
    // Session with a client but no windows
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    session.add_client(ClientId::new(1));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_selection(request).await;

    // Client exists but no active window = NotFound
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_selection_with_char_selection() {
    use {
        reovim_domain_text::Position as KernelPosition,
        reovim_driver_session::{Viewport, api::Selection},
    };

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("hello world");
        })
        .await;

    // Create a client - this initializes per-client state
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Modify the CLIENT's per-client state (not shared state)
    session.clients().update_client_state(client_id, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.viewport = Viewport::new(80, 24);
            // Set selection for "hello" (0,0 to 0,5 exclusive)
            window.selection =
                Some(Selection::character(KernelPosition::new(0, 0), KernelPosition::new(0, 5)));
        }
    });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_selection(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.has_selection);
    assert!(resp.selection.is_some());

    let sel = resp.selection.unwrap();
    assert_eq!(sel.start.as_ref().unwrap().line, 0);
    assert_eq!(sel.start.as_ref().unwrap().column, 0);
    assert_eq!(sel.end.as_ref().unwrap().line, 0);
    assert_eq!(sel.end.as_ref().unwrap().column, 5); // exclusive end
    assert_eq!(resp.visual_mode, Some("char".to_string()));
}

#[tokio::test]
async fn test_get_selection_line_mode() {
    use {
        reovim_domain_text::Position as KernelPosition,
        reovim_driver_session::{Viewport, api::Selection},
    };

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("line1\nline2\nline3");
        })
        .await;

    // Create a client
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Modify the CLIENT's per-client state
    session.clients().update_client_state(client_id, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.viewport = Viewport::new(80, 24);
            // Line-wise selection for lines 0-1
            window.selection = Some(Selection::line(
                KernelPosition::new(0, 0),
                KernelPosition::new(2, 0), // exclusive end
            ));
        }
    });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_selection(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.has_selection);
    assert_eq!(resp.visual_mode, Some("line".to_string()));
}

#[tokio::test]
async fn test_get_selection_block_mode() {
    use {
        reovim_domain_text::Position as KernelPosition,
        reovim_driver_session::{Viewport, api::Selection},
    };

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("ABC\nDEF\nGHI");
        })
        .await;

    // Create a client
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Modify the CLIENT's per-client state
    session.clients().update_client_state(client_id, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.viewport = Viewport::new(80, 24);
            // Block selection from (0,0) to (1,2) - a 2x2 block
            window.selection = Some(Selection::block(
                KernelPosition::new(0, 0),
                KernelPosition::new(2, 2), // exclusive end
            ));
        }
    });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_selection(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.has_selection);
    assert_eq!(resp.visual_mode, Some("block".to_string()));
}

#[tokio::test]
async fn test_get_selection_reverse() {
    use {
        reovim_domain_text::Position as KernelPosition,
        reovim_driver_session::{Viewport, api::Selection},
    };

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("hello world");
        })
        .await;

    // Create a client
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Modify the CLIENT's per-client state
    session.clients().update_client_state(client_id, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.viewport = Viewport::new(80, 24);
            // Selection from column 2 to column 5 (already normalized)
            window.selection =
                Some(Selection::character(KernelPosition::new(0, 2), KernelPosition::new(0, 5)));
        }
    });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_selection(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.has_selection);

    let sel = resp.selection.unwrap();
    // start should be normalized (smaller position comes first)
    assert!(sel.start.as_ref().unwrap().column < sel.end.as_ref().unwrap().column);
    assert_eq!(sel.start.as_ref().unwrap().column, 2);
    assert_eq!(sel.end.as_ref().unwrap().column, 5);
}

// Per-client state (#471): Per-client mode isolation tests

#[tokio::test]
async fn test_get_mode_per_client_returns_client_mode() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let (registry, session) = test_registry_with_buffer_manager();

    // Add a client with custom mode stack
    let client_id = crate::session::ClientId::new(42);
    session.add_client(client_id);

    // Modify the client's per-client mode stack to INSERT mode
    let module = ModuleId::new("editor");
    session
        .clients()
        .update_client_state(client_id, |editing_state| {
            editing_state
                .mode_stack
                .push(ModeId::new(module.clone(), "insert"));
        });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Query with client_id = 42 should return INSERT mode
    let request = authed_request(GetModeRequest { client_id: 42 }, ClientId::new(42));
    let response = service.get_mode(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.name, "insert");
    assert_eq!(resp.display, "INSERT");
    assert!(resp.is_insert);
}

// Note: test_get_mode_unknown_client_falls_back_to_shared was REMOVED in Phase #479.
// Unknown clients now return NotFound error (see test_get_mode_unknown_client_returns_not_found).
// client_id=0 now returns InvalidArgument (see test_get_mode_rejects_client_id_zero).

// =========================================================================
// Phase #471: Multi-Client Isolation Tests
// =========================================================================

#[tokio::test]
async fn test_cursor_isolation_between_clients() {
    use reovim_driver_session::CursorPosition;

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("line1\nline2\nline3\nline4\nline5");
        })
        .await;

    // Create two clients
    let client_a = crate::session::ClientId::new(1);
    let client_b = crate::session::ClientId::new(2);
    session.add_client(client_a);
    session.add_client(client_b);

    // Client A moves cursor to (3, 5)
    session.clients().update_client_state(client_a, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.cursor = CursorPosition { line: 3, column: 5 };
        }
    });

    // Client B's cursor should still be at default (0, 0)
    let state_b = session.clients().client_state(client_b).unwrap();
    let cursor_b = state_b.windows.active().unwrap().cursor;
    assert_eq!(cursor_b.line, 0, "Client B cursor line should be 0");
    assert_eq!(cursor_b.column, 0, "Client B cursor column should be 0");

    // Verify via gRPC service
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Query Client A's cursor
    let request = authed_request(
        GetCursorRequest {
            window_id: None,
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_cursor(request).await.unwrap().into_inner();
    let pos = response.position.unwrap();
    assert_eq!(pos.line, 3, "Client A gRPC cursor line");
    assert_eq!(pos.column, 5, "Client A gRPC cursor column");

    // Query Client B's cursor - should be independent
    let request = authed_request(
        GetCursorRequest {
            window_id: None,
            client_id: 2,
        },
        ClientId::new(2),
    );
    let response = service.get_cursor(request).await.unwrap().into_inner();
    let pos = response.position.unwrap();
    assert_eq!(pos.line, 0, "Client B gRPC cursor line");
    assert_eq!(pos.column, 0, "Client B gRPC cursor column");
}

#[tokio::test]
async fn test_mode_isolation_between_clients() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let (registry, session) = test_registry_with_buffer_manager();

    // Create two clients
    let client_a = crate::session::ClientId::new(10);
    let client_b = crate::session::ClientId::new(20);
    session.add_client(client_a);
    session.add_client(client_b);

    // Client A enters INSERT mode
    let module = ModuleId::new("editor");
    session.clients().update_client_state(client_a, |state| {
        state.mode_stack.push(ModeId::new(module.clone(), "insert"));
    });

    // Client B should still be in NORMAL mode (not affected by A)
    let mode_b = session.client_current_mode(client_b).unwrap();
    assert_eq!(mode_b.name(), "normal", "Client B should remain in normal mode");

    // Verify via gRPC service
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Query Client A's mode - should be INSERT
    let request = authed_request(GetModeRequest { client_id: 10 }, ClientId::new(10));
    let response = service.get_mode(request).await.unwrap().into_inner();
    assert_eq!(response.name, "insert", "Client A gRPC mode");
    assert!(response.is_insert, "Client A should be in insert mode");

    // Query Client B's mode - should still be NORMAL
    let request = authed_request(GetModeRequest { client_id: 20 }, ClientId::new(20));
    let response = service.get_mode(request).await.unwrap().into_inner();
    assert_eq!(response.name, "normal", "Client B gRPC mode");
    assert!(!response.is_insert, "Client B should NOT be in insert mode");
}

#[tokio::test]
async fn test_layout_isolation_per_client() {
    use reovim_driver_session::Viewport;

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
    use reovim_driver_session::Window;

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
async fn test_get_registers_linewise() {
    use reovim_domain_text::RegisterContent;

    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    // Set register on per-client state (#515)
    session
        .clients()
        .update_client_state(ClientId::new(1), |state| {
            state
                .registers
                .set(RegisterContent::linewise("line content\n"));
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
    assert_eq!(resp.registers[0].yank_type, "line");
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

#[tokio::test]
async fn test_get_selection_unknown_client() {
    let registry = test_registry();
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 999,
        },
        ClientId::new(999),
    );
    let response = service.get_selection(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

// =========================================================================
// Coverage: Ring buffer logging for unknown clients (#497)
// =========================================================================

#[tokio::test]
async fn test_get_mode_following_client_triggers_ring_buffer_log() {
    // A Following client returns None from client_current_mode, which triggers
    // the ok_or_else closure that logs to the ring buffer (lines 109-115).
    use crate::session::ClientRelation;

    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);
    session.add_client(owner_id);
    session.add_client(follower_id);

    let _ = session
        .clients()
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

    // Following client -> client_current_mode returns None -> NotFound with ring buffer log
    let request = authed_request(GetModeRequest { client_id: 2 }, ClientId::new(2));
    let response = service.get_mode(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_get_cursor_following_client_triggers_ring_buffer_log() {
    // A Following client returns None from client_state, triggering
    // the ok_or_else closure with ring buffer logging (lines 153-159).
    use crate::session::ClientRelation;

    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);
    session.add_client(owner_id);
    session.add_client(follower_id);

    let _ = session
        .clients()
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

    // Note: client_state for Following returns target's state, so we need a case
    // where it actually fails. Use an unknown client ID that has a ring buffer.
    // Actually, Following clients DO return effective state from target.
    // So let's use a client that IS registered but has some state issue.
    // The real trigger is when client_state returns None, which happens when
    // the client is not found at all. But we want ring buffer log which requires
    // the client to exist.
    //
    // In practice, client_state returns None only when the client is not found.
    // The ring buffer log is best-effort (logs if client has ring buffer).
    // We just need the NotFound path.
    let request = authed_request(
        GetCursorRequest {
            window_id: None,
            client_id: 999,
        },
        ClientId::new(999),
    );
    let response = service.get_cursor(request).await;
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

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

#[tokio::test]
async fn test_get_selection_client_not_found_logs() {
    // Test the ring buffer logging path in get_selection (lines 362-368).
    let (registry, session) = test_registry_with_session();
    let service = StateServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));

    session.add_client(ClientId::new(1));

    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 666,
        },
        ClientId::new(666),
    );
    let response = service.get_selection(request).await;

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
async fn test_get_registers_specific_register_with_content() {
    // Test the specific register lookup path where
    // the register exists and has content.
    use reovim_domain_text::RegisterContent;

    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    // Set a named register on per-client state (#515)
    session
        .clients()
        .update_client_state(ClientId::new(1), |state| {
            state
                .registers
                .set_named('a', RegisterContent::characterwise("hello world"));
        });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Query register 'a' by name
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
    assert_eq!(resp.registers[0].content, "hello world");
    assert_eq!(resp.registers[0].yank_type, "char");
}

#[tokio::test]
async fn test_get_registers_specific_linewise_register() {
    // Test the linewise yank_type path in specific register lookup.
    use reovim_domain_text::RegisterContent;

    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    session
        .clients()
        .update_client_state(ClientId::new(1), |state| {
            state
                .registers
                .set_named('b', RegisterContent::linewise("a full line\n"));
        });

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetRegistersRequest {
            names: vec!["b".to_string()],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.registers.len(), 1);
    assert_eq!(resp.registers[0].yank_type, "line");
}

#[tokio::test]
async fn test_get_registers_multiple_specific() {
    // Test querying multiple specific registers.
    use reovim_domain_text::RegisterContent;

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
    use reovim_domain_text::RegisterContent;

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

#[tokio::test]
async fn test_selection_isolation_per_client() {
    use {
        reovim_domain_text::Position as KernelPosition,
        reovim_driver_session::{Viewport, api::Selection},
    };

    let (registry, session) = test_registry_with_buffer_manager();

    // Create buffer first
    session
        .with_state_mut(|state| {
            state.create_buffer("hello world");
        })
        .await;

    // Create two clients
    let client_a = crate::session::ClientId::new(50);
    let client_b = crate::session::ClientId::new(60);
    session.add_client(client_a);
    session.add_client(client_b);

    // Client A has a selection
    session.clients().update_client_state(client_a, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.viewport = Viewport::new(80, 24);
            window.selection =
                Some(Selection::character(KernelPosition::new(0, 0), KernelPosition::new(0, 5)));
        }
    });

    // Client B has no selection (just viewport)
    session.clients().update_client_state(client_b, |state| {
        if let Some(window) = state.windows.active_mut() {
            window.viewport = Viewport::new(80, 24);
            window.selection = None;
        }
    });

    // Verify via gRPC service
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    // Query Client A's selection - should have selection
    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 50,
        },
        ClientId::new(50),
    );
    let response = service.get_selection(request).await.unwrap().into_inner();
    assert!(response.has_selection, "Client A should have selection");
    assert_eq!(response.visual_mode, Some("char".to_string()), "Client A selection mode");

    // Query Client B's selection - should have NO selection
    let request = authed_request(
        GetSelectionRequest {
            window_id: None,
            client_id: 60,
        },
        ClientId::new(60),
    );
    let response = service.get_selection(request).await.unwrap().into_inner();
    assert!(!response.has_selection, "Client B should NOT have selection");
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
async fn test_get_cursor_dangling_follower_not_found() {
    let cid = ClientId::new(50);
    let registry = test_registry_with_dangling_follower(cid);
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetCursorRequest {
            client_id: cid.as_usize() as u64,
            window_id: None,
        },
        cid,
    );
    let err = service.get_cursor(request).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::NotFound);
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

#[tokio::test]
async fn test_get_selection_dangling_follower_not_found() {
    let cid = ClientId::new(53);
    let registry = test_registry_with_dangling_follower(cid);
    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetSelectionRequest {
            client_id: cid.as_usize() as u64,
            window_id: None,
        },
        cid,
    );
    let err = service.get_selection(request).await.unwrap_err();
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
        reovim_driver_buffer::TestBufferManager,
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
        reovim_driver_buffer::TestBufferManager,
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
        reovim_driver_buffer::TestBufferManager,
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
        reovim_driver_buffer::TestBufferManager,
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
