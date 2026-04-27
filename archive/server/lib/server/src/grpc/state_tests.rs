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
    use reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry};

    // Create a KernelContext with a real buffer manager
    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_kernel::testing::TestBufferManager::new()),
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

/// (#753) `get_projections` replaces `get_mode`, `get_cursor`, `get_selection`.
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

// test_get_registers_with_content: registers are domain-owned (#753 E3).
// get_registers now returns empty; domain driver will supply content via projections.
#[tokio::test]
async fn test_get_registers_with_content() {
    let (registry, session) = test_registry_with_buffer_manager();
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
    // Stub: always empty (#753 E3)
    let resp = response.unwrap().into_inner();
    assert!(resp.registers.is_empty());
}

// test_get_registers_specific_register: registers domain-owned (#753 E3), stub returns empty.
#[tokio::test]
async fn test_get_registers_specific_register() {
    let (registry, session) = test_registry_with_buffer_manager();
    session.add_client(ClientId::new(1));

    let service = StateServiceImpl::new(registry, SessionId::new("test"));

    let request = authed_request(
        GetRegistersRequest {
            names: vec!["a".to_string()],
            client_id: 1,
        },
        ClientId::new(1),
    );
    let response = service.get_registers(request).await;

    assert!(response.is_ok());
    // Stub: always empty (#753 E3)
    let resp = response.unwrap().into_inner();
    assert!(resp.registers.is_empty());
}

// (#753) GetMode/GetCursor/GetSelection replaced by GetProjections.
// Cursor, mode, and selection state is now domain-neutral (ProjectionUpdated notifications).

// test_layout_isolation_per_client: viewport/windows domain-owned (#753 E3).
// Layout now uses compositor placements; without compositor the root is None.
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

// test_get_registers_multiple_specific removed: registers are domain-owned (#753 E3)
// test_get_registers_specific_empty_register_filtered_out removed: registers are domain-owned (#753 E3)

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

// =========================================================================
// Coverage: kernel_to_proto_option() all variants (L62-77)
// =========================================================================

#[test]
fn test_kernel_to_proto_option_bool_true() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v3::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::Bool(true));
    assert_eq!(proto.value, Some(Value::BoolValue(true)));
}

#[test]
fn test_kernel_to_proto_option_bool_false() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v3::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::Bool(false));
    assert_eq!(proto.value, Some(Value::BoolValue(false)));
}

#[test]
fn test_kernel_to_proto_option_integer() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v3::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::Integer(42));
    assert_eq!(proto.value, Some(Value::IntValue(42)));
}

#[test]
fn test_kernel_to_proto_option_string() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v3::option_value::Value,
    };

    let proto = kernel_to_proto_option(&KernelOptionValue::String("hello".to_string()));
    assert_eq!(proto.value, Some(Value::StringValue("hello".to_string())));
}

#[test]
fn test_kernel_to_proto_option_choice() {
    use {
        super::kernel_to_proto_option, reovim_kernel::api::v1::OptionValue as KernelOptionValue,
        reovim_protocol::v3::option_value::Value,
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
    use reovim_kernel::api::v1::{
        EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
    };

    // Build a session with a real OptionRegistry that has a registered option.
    let option_registry = Arc::new(OptionRegistry::new());
    option_registry
        .register(OptionSpec::new("number", "Show line numbers", OptionValue::Bool(false)))
        .expect("register option");

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_kernel::testing::TestBufferManager::new()),
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
    assert_eq!(opt.value, Some(reovim_protocol::v3::option_value::Value::BoolValue(false)));
}

#[tokio::test]
async fn test_get_options_with_alias_resolves() {
    use reovim_kernel::api::v1::{
        EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
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
        Arc::new(reovim_kernel::testing::TestBufferManager::new()),
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
    use reovim_kernel::api::v1::{
        EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
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
        Arc::new(reovim_kernel::testing::TestBufferManager::new()),
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
    use reovim_kernel::api::v1::{
        EventBus, KernelContext, OptionRegistry, OptionSpec, OptionValue, ServiceRegistry,
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
        Arc::new(reovim_kernel::testing::TestBufferManager::new()),
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
