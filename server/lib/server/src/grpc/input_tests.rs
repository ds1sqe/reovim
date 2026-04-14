use {
    super::*,
    crate::session::SessionState,
    parking_lot::Mutex,
    reovim_driver_codec::{
        CodecMetadata, ContentCodec, ContentCodecFactory, ContentCodecFactoryStore, ContentType,
        DecodeResult,
    },
    reovim_driver_text_input::TransitionContext,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
    std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    },
};

// === Helpers for codec routing tests in this module ===

#[derive(Clone)]
struct TestTrackedCodec {
    calls: Arc<AtomicUsize>,
    byte_edit: Vec<u8>,
}

impl ContentCodec for TestTrackedCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, reovim_driver_codec::CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/codec-route")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        _bytes: &dyn reovim_subsys_vfs::ByteSource,
        _edit: &reovim_driver_codec::DecodedEdit,
    ) -> Result<Option<ByteEdit>, reovim_driver_codec::TranslateEditError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(ByteEdit::insert(0, &self.byte_edit)))
    }
}

#[derive(Clone)]
struct TestCodecFactory {
    content_type: &'static str,
    calls: Arc<AtomicUsize>,
    byte_edit: Vec<u8>,
}

impl ContentCodecFactory for TestCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == self.content_type {
            Some(Arc::new(TestTrackedCodec {
                calls: Arc::clone(&self.calls),
                byte_edit: self.byte_edit.clone(),
            }))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![self.content_type]
    }

    fn name(&self) -> &'static str {
        "tracking"
    }
}

fn make_codec_session(
    content_type: &'static str,
    translate_to: &[u8],
) -> (Arc<Session>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let state = SessionState::default();

    let factories = ContentCodecFactoryStore::new();
    factories.add_factory(Arc::new(TestCodecFactory {
        content_type,
        calls: Arc::clone(&calls),
        byte_edit: translate_to.to_vec(),
    }));
    state.app.kernel.services.register(Arc::new(factories));

    (Arc::new(Session::from_state(SessionId::new("test"), state)), calls)
}

fn record_codec_buffer(
    session: &Arc<Session>,
    buffer_id: BufferId,
    source: &[u8],
    content_type: &str,
) {
    session.with_state_mut_sync(|state| {
        use reovim_driver_codec::{ContentCodecFactoryStore, ContentType};

        let codec_state = state
            .app
            .extensions
            .get_or_insert::<reovim_driver_codec::CodecSessionState>();
        codec_state.insert(buffer_id, CodecMetadata::new(ContentType::new(content_type)));
        let codec = state
            .app
            .kernel
            .services
            .get::<ContentCodecFactoryStore>()
            .and_then(|store| store.find(&ContentType::new(content_type)));

        if let Some(codec) = codec {
            codec_state.set_source_with_codec(buffer_id, source.to_vec(), codec);
        } else {
            codec_state.set_source(buffer_id, source.to_vec());
        }
    });
}

/// Helper: create a fresh snapshot cache for test isolation.
fn test_cache() -> Mutex<HashMap<(String, u64), String>> {
    Mutex::new(HashMap::new())
}

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

/// Helper: build a request with token-authenticated `ClientId` in extensions.
fn authed_request<T>(body: T, client_id: ClientId) -> Request<T> {
    let mut request = Request::new(body);
    request.extensions_mut().insert(client_id);
    request
}

#[tokio::test]
#[cfg_attr(coverage_nightly, coverage(off))]
#[ignore = "Per-client state (#471): Requires resolver registration; panics without modules"]
async fn test_send_keys_valid_notation() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(
        SendKeysRequest {
            keys: "abc".to_string(),
        },
        ClientId::new(1),
    );
    let response = service.send_keys(request).await;

    // Should parse successfully, but may not execute without active buffer
    assert!(response.is_ok());
}

#[tokio::test]
async fn test_send_keys_invalid_notation() {
    let registry = test_registry();
    // Client must exist (created via Join() in production)
    registry
        .get(&SessionId::new("test"))
        .unwrap()
        .add_client(ClientId::new(1));
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(
        SendKeysRequest {
            keys: "<Ctrl".to_string(),
        },
        ClientId::new(1),
    );
    let response = service.send_keys(request).await;

    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
#[cfg_attr(coverage_nightly, coverage(off))]
#[ignore = "Per-client state (#471): Requires resolver registration; panics without modules"]
async fn test_send_keys_special_keys() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(
        SendKeysRequest {
            keys: "<Esc>".to_string(),
        },
        ClientId::new(1),
    );
    let response = service.send_keys(request).await;

    // Should parse but not handle (non-character key)
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok); // Not handled in minimal impl
}

#[tokio::test]
#[cfg_attr(coverage_nightly, coverage(off))]
#[ignore = "Per-client state (#471): Requires resolver registration; panics without modules"]
async fn test_send_keys_with_modifiers() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(
        SendKeysRequest {
            keys: "<C-w>".to_string(),
        },
        ClientId::new(1),
    );
    let response = service.send_keys(request).await;

    // Should parse but not handle (modifier key)
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok); // Not handled in minimal impl
}

#[tokio::test]
async fn test_send_keys_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    // No session inserted
    let service = InputServiceImpl::new(
        registry,
        SessionId::new("nonexistent"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        ClientId::new(1),
    );
    let response = service.send_keys(request).await;

    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_send_keys_rejects_unauthenticated() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // No ClientId in extensions — should be rejected
    let request = Request::new(SendKeysRequest {
        keys: "a".to_string(),
    });
    let response = service.send_keys(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[test]
fn test_resolve_to_command_context_empty() {
    let ctx = ResolveContext::default();
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    // Empty context should produce empty command context
    assert!(cmd_ctx.count().is_none());
    assert!(cmd_ctx.register().is_none());
}

#[test]
fn test_resolve_to_command_context_with_count() {
    let ctx = ResolveContext {
        count: Some(5),
        ..ResolveContext::default()
    };
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);

    // Should have count set
    assert_eq!(cmd_ctx.count(), Some(5));
}

#[test]
fn test_resolve_to_command_context_with_register() {
    let ctx = ResolveContext {
        register: Some('a'),
        ..ResolveContext::default()
    };
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);

    // Should have register set
    assert_eq!(cmd_ctx.register(), Some('a'));
}

#[test]
fn test_resolve_to_command_context_with_count_and_register() {
    let ctx = ResolveContext {
        count: Some(3),
        register: Some('"'),
        ..ResolveContext::default()
    };
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);

    assert_eq!(cmd_ctx.count(), Some(3));
    assert_eq!(cmd_ctx.register(), Some('"'));
}

#[test]
fn test_resolve_to_command_context_with_metadata() {
    let mut ctx = ResolveContext::default();
    ctx.metadata.insert(
        "test_key".to_string(),
        reovim_driver_text_input::ArgValue::String("test_value".to_string()),
    );
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);

    // Metadata is transferred with type conversion
    assert_eq!(cmd_ctx.string("test_key"), Some("test_value"),);
}

#[tokio::test]
async fn test_send_keys_client_not_found() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // Client 42 is not added to session
    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        ClientId::new(42),
    );
    let response = service.send_keys(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn test_send_keys_following_client_ignored() {
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();

    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);
    session.add_client(owner_id);
    session.add_client(follower_id);

    // Set follower relation
    let _ = session.clients().set_client_relation(
        follower_id,
        Some(crate::session::ClientRelation::Following { target: owner_id }),
    );

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        follower_id,
    );
    let response = service.send_keys(request).await;

    // Following clients should have their input ignored
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok);
    assert_eq!(resp.status, i32::from(KeyStatus::NotFound));
}

#[test]
fn test_emit_notifications_with_empty_changes() {
    let session = crate::session::Session::new(SessionId::new("emit-test"));
    let changes = StateChanges::new();
    // Should not panic even with empty changes
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );
}

#[test]
fn test_emit_notifications_with_mode_change() {
    let session = crate::session::Session::new(SessionId::new("emit-mode-test"));
    let mut changes = StateChanges::new();
    changes.record_mode_change();

    // Should emit mode_changed notification without panic
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );
}

#[test]
fn test_emit_notifications_with_buffer_modified() {
    let session = crate::session::Session::new(SessionId::new("emit-buf-test"));
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buffer_id);

    // Subscribe to verify notifications are emitted
    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    let received = rx.try_recv();
    assert!(received.is_ok());
    assert_eq!(received.unwrap().event_type, "buffer_modified");
}

#[test]
fn test_emit_notifications_with_multiple_changes() {
    let session = crate::session::Session::new(SessionId::new("emit-multi-test"));
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(5);
    let mut changes = StateChanges::new();
    changes.record_mode_change();
    changes.record_buffer_modified(buffer_id);
    changes.buffers_created.push(buffer_id);

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    // Should receive multiple notifications
    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    assert_eq!(count, 3); // mode_changed + buffer_modified + buffer_list_changed
}

#[tokio::test]
async fn test_apply_mode_transition_push() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("push-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let transition = ModeTransition::Push {
        mode: insert_mode.clone(),
        context: TransitionContext::new(),
    };

    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    // Client should now be in insert mode
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "insert");
}

#[tokio::test]
async fn test_apply_mode_transition_set() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("set-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // First push a mode
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
    });

    // Now set to a different mode
    let visual_mode = ModeId::new(ModuleId::new("test"), "visual");
    let transition = ModeTransition::Set {
        mode: visual_mode.clone(),
        context: TransitionContext::new(),
    };

    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    // Client should now be in visual mode (stack reset to base then set)
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "visual");
}

#[tokio::test]
async fn test_apply_mode_transition_pop() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("pop-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push a mode first
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
    });

    // Pop should go back to normal
    let transition = ModeTransition::Pop { result: None };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

#[tokio::test]
async fn test_apply_mode_transition_pop_at_base_is_noop() {
    let session = crate::session::Session::new(SessionId::new("pop-base-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Mode stack has only base mode, pop should be a no-op
    let transition = ModeTransition::Pop { result: None };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    // Should still be in normal mode
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

#[tokio::test]
async fn test_apply_mode_transition_on_nonexistent_client() {
    let session = crate::session::Session::new(SessionId::new("no-client-test"));
    let nonexistent = ClientId::new(999);

    let transition = ModeTransition::Pop { result: None };
    // Should not panic, just log a warning
    InputServiceImpl::apply_mode_transition_for_client(&session, nonexistent, transition).await;
}

#[test]
fn test_handle_pop_result_cancelled() {
    let session = crate::session::Session::new(SessionId::new("pop-cancelled-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Should not panic
    InputServiceImpl::handle_pop_result_for_client(&session, client_id, PopResult::Cancelled);
}

#[test]
fn test_handle_pop_result_data() {
    let session = crate::session::Session::new(SessionId::new("pop-data-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Should not panic
    InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::Data {
            values: HashMap::from([(
                "key".to_string(),
                reovim_subsys_command_types::ArgValue::String("value".to_string()),
            )]),
        },
    );
}

#[test]
fn test_handle_pop_result_execute_command_nonexistent() {
    let session = crate::session::Session::new(SessionId::new("pop-exec-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "nonexistent",
    );

    // Should not panic - command not found returns None
    InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args: HashMap::new(),
        },
    );
}

#[test]
fn test_resolve_to_command_context_with_all_fields() {
    let mut ctx = ResolveContext {
        count: Some(10),
        register: Some('z'),
        ..ResolveContext::default()
    };
    ctx.metadata.insert(
        "motion".to_string(),
        reovim_driver_text_input::ArgValue::String("word".to_string()),
    );
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);

    assert_eq!(cmd_ctx.count(), Some(10));
    assert_eq!(cmd_ctx.register(), Some('z'));
}

#[test]
fn test_handle_pop_result_execute_command_with_args() {
    let session = crate::session::Session::new(SessionId::new("pop-args-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "some-cmd",
    );

    let mut args = HashMap::new();
    args.insert("count".to_string(), reovim_subsys_command_types::ArgValue::Count(5));
    args.insert("register".to_string(), reovim_subsys_command_types::ArgValue::Register('a'));
    args.insert(
        "description".to_string(),
        reovim_subsys_command_types::ArgValue::String("test arg".to_string()),
    );

    // Should not panic - command not found, but args are processed
    InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args,
        },
    );
}

#[test]
fn test_emit_notifications_with_cursor_moved() {
    let session = crate::session::Session::new(SessionId::new("emit-cursor-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Set up a window displaying the buffer for this client
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    session.clients().update_client_state(client_id, |state| {
        let window = reovim_driver_text_session::Window::with_buffer(buffer_id);
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    let mut changes = StateChanges::new();
    changes.record_cursor_move(buffer_id);

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        1,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    let received = rx.try_recv();
    assert!(received.is_ok());
    assert_eq!(received.unwrap().event_type, "cursor_moved");
}

#[test]
fn test_emit_notifications_with_selection_changed() {
    let session = crate::session::Session::new(SessionId::new("emit-sel-test"));
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    changes.selection_changed = true;
    changes.affected_buffers.push(buffer_id);

    // Emit notifications for selection changes
    // No client state set up, so selection notification may not produce output
    // but should not panic
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );
}

#[test]
fn test_emit_notifications_with_window_changed() {
    let session = crate::session::Session::new(SessionId::new("emit-layout-test"));
    let mut changes = StateChanges::new();
    changes.window_changed = true;

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    let received = rx.try_recv();
    assert!(received.is_ok());
    assert_eq!(received.unwrap().event_type, "layout_changed");
}

#[test]
fn test_emit_notifications_with_buffer_list_changed() {
    let session = crate::session::Session::new(SessionId::new("emit-buflist-test"));
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(3);
    let mut changes = StateChanges::new();
    changes.buffers_created.push(buffer_id);

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    let received = rx.try_recv();
    assert!(received.is_ok());
    assert_eq!(received.unwrap().event_type, "buffer_list_changed");
}

#[test]
fn test_emit_notifications_with_option_changed() {
    use reovim_driver_text_session::api::OptionChange;

    let session = crate::session::Session::new(SessionId::new("emit-option-test"));
    let mut changes = StateChanges::new();
    changes.options_changed.push(OptionChange {
        name: "virtualedit".to_string(),
        value: reovim_kernel::api::v1::OptionValue::String("all".to_string()),
        window_id: None,
    });

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    let received = rx.try_recv();
    assert!(received.is_ok());
    assert_eq!(received.unwrap().event_type, "option_changed");
}

#[test]
fn test_emit_notifications_with_scroll_changed() {
    let session = crate::session::Session::new(SessionId::new("emit-viewport-test"));
    let window_id = reovim_kernel::api::v1::WindowId::new();
    let mut changes = StateChanges::new();
    changes.scroll_changed = true;
    changes.scrolled_windows.push(window_id);

    // No client windows means viewport notification won't be produced
    // but should not panic
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );
}

#[test]
fn test_emit_notifications_all_change_types_at_once() {
    use reovim_driver_text_session::api::OptionChange;

    let session = crate::session::Session::new(SessionId::new("emit-all-test"));
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();

    changes.record_mode_change();
    changes.record_cursor_move(buffer_id);
    changes.record_buffer_modified(buffer_id);
    changes.window_changed = true;
    changes.buffers_created.push(buffer_id);
    changes.options_changed.push(OptionChange {
        name: "opt".to_string(),
        value: reovim_kernel::api::v1::OptionValue::Bool(true),
        window_id: None,
    });

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        42,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    // Should have multiple notifications
    assert!(count >= 4, "Expected at least 4 notifications, got {count}");
}

#[tokio::test]
async fn test_apply_mode_transition_push_multiple_modes() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("push-multi-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push insert mode
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let transition1 = ModeTransition::Push {
        mode: insert_mode.clone(),
        context: TransitionContext::new(),
    };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition1).await;

    // Push visual mode on top
    let visual_mode = ModeId::new(ModuleId::new("test"), "visual");
    let transition2 = ModeTransition::Push {
        mode: visual_mode,
        context: TransitionContext::new(),
    };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition2).await;

    // Should be in visual mode
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "visual");
}

#[tokio::test]
async fn test_apply_mode_transition_pop_with_result_cancelled() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("pop-cancel-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push a mode first
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
    });

    // Pop with cancelled result
    let transition = ModeTransition::Pop {
        result: Some(PopResult::Cancelled),
    };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    // Should have popped back to normal
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

#[tokio::test]
async fn test_apply_mode_transition_pop_with_data_result() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("pop-data-result-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push a mode first
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
    });

    // Pop with data result
    let transition = ModeTransition::Pop {
        result: Some(PopResult::Data {
            values: HashMap::from([(
                "search".to_string(),
                reovim_subsys_command_types::ArgValue::String("pattern".to_string()),
            )]),
        }),
    };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

#[tokio::test]
async fn test_apply_mode_transition_pop_with_execute_command_result() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("pop-exec-result-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push a mode first
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
    });

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "delete-range",
    );

    // Pop with execute command result
    let transition = ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand {
            command: cmd_id,
            args: HashMap::new(),
        }),
    };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

#[tokio::test]
async fn test_apply_mode_transition_set_clears_stack() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("set-clear-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push multiple modes
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let visual_mode = ModeId::new(ModuleId::new("test"), "visual");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
        state.mode_stack.push(visual_mode);
    });

    // Set should pop all and set the new mode
    let cmd_mode = ModeId::new(ModuleId::new("test"), "command");
    let transition = ModeTransition::Set {
        mode: cmd_mode.clone(),
        context: TransitionContext::new(),
    };
    InputServiceImpl::apply_mode_transition_for_client(&session, client_id, transition).await;

    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "command");
}

#[test]
fn test_resolve_to_command_context_no_count_no_register() {
    let ctx = ResolveContext {
        count: None,
        register: None,
        ..ResolveContext::default()
    };
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    assert!(cmd_ctx.count().is_none());
    assert!(cmd_ctx.register().is_none());
}

#[test]
fn test_resolve_to_command_context_with_multiple_metadata() {
    let mut ctx = ResolveContext::default();
    ctx.metadata.insert(
        "key1".to_string(),
        reovim_driver_text_input::ArgValue::String("val1".to_string()),
    );
    ctx.metadata.insert(
        "key2".to_string(),
        reovim_driver_text_input::ArgValue::String("val2".to_string()),
    );
    ctx.metadata.insert(
        "key3".to_string(),
        reovim_driver_text_input::ArgValue::String("val3".to_string()),
    );
    // Should handle multiple metadata entries without panic
    let _cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
}

#[test]
fn test_input_service_impl_new() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));
    // get_session should succeed
    assert!(service.get_session().is_ok());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_input_service_impl_get_session_not_found() {
    let registry = Arc::new(SessionRegistry::new());
    let service = InputServiceImpl::new(
        registry,
        SessionId::new("nonexistent"),
        Arc::new(BridgeRegistry::new()),
    );
    match service.get_session() {
        Ok(_) => panic!("Expected NotFound error"),
        Err(err) => assert_eq!(err.code(), tonic::Code::NotFound),
    }
}

#[test]
fn test_emit_notifications_with_client_id() {
    let session = crate::session::Session::new(SessionId::new("emit-cid-test"));
    let mut changes = StateChanges::new();
    changes.record_mode_change();

    // Should work with any client_id value
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        42,
        &BridgeRegistry::new(),
        &test_cache(),
    );
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        u64::MAX,
        &BridgeRegistry::new(),
        &test_cache(),
    );
}

#[test]
fn test_handle_pop_result_execute_command_with_active_buffer() {
    use reovim_kernel::api::v1::BufferId;

    let session = crate::session::Session::new(SessionId::new("pop-buf-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Set active buffer via per-client state (#471)
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(BufferId::from_raw(5));
    });

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "delete-range",
    );

    // Execute with args and active buffer
    InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args: HashMap::from([(
                "range".to_string(),
                reovim_subsys_command_types::ArgValue::String("1,5".to_string()),
            )]),
        },
    );
}

#[tokio::test]
async fn test_handle_resolve_result_completed() {
    use reovim_driver_text_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("completed-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('x'));

    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::Completed,
        &key,
        client_id,
    )
    .await;

    assert!(handled);
    assert!(!changes.has_changes());
}

#[tokio::test]
async fn test_handle_resolve_result_pending() {
    use reovim_driver_text_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("pending-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('d'));

    let (handled, changes) =
        InputServiceImpl::handle_resolve_result(&session, ResolveResult::Pending, &key, client_id)
            .await;

    assert!(handled);
    assert!(!changes.has_changes());
}

#[tokio::test]
async fn test_handle_resolve_result_not_handled() {
    use reovim_driver_text_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("nothandled-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('z'));

    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::NotHandled,
        &key,
        client_id,
    )
    .await;

    assert!(!handled);
    assert!(!changes.has_changes());
}

#[tokio::test]
async fn test_handle_resolve_result_insert_char() {
    use reovim_driver_text_input::{InputTarget, KeyCode};

    let session = crate::session::Session::new(SessionId::new("insertchar-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('a'));

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InsertChar {
            char: 'a',
            target: InputTarget::Buffer,
        },
        &key,
        client_id,
    )
    .await;

    assert!(handled);
}

#[tokio::test]
async fn test_handle_resolve_result_mode_transition() {
    use {
        reovim_driver_text_input::KeyCode,
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    let session = crate::session::Session::new(SessionId::new("modetrans-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('i'));

    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let transition = ModeTransition::Push {
        mode: insert_mode,
        context: TransitionContext::new(),
    };

    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::ModeTransition(transition),
        &key,
        client_id,
    )
    .await;

    assert!(handled);
    assert!(changes.mode_changed);
}

#[tokio::test]
async fn test_handle_resolve_result_inject_keys_empty() {
    use reovim_driver_text_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("inject-empty-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('q'));

    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InjectKeys {
            keys: vec![],
            exit_macro_playback: false,
        },
        &key,
        client_id,
    )
    .await;

    assert!(handled);
    assert!(!changes.has_changes());
}

#[test]
fn test_emit_notifications_has_changes_check() {
    let session = crate::session::Session::new(SessionId::new("haschanges-test"));
    let changes = StateChanges::new();

    // Empty changes should not emit
    assert!(!changes.has_changes());

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    // Should not receive any notifications
    assert!(rx.try_recv().is_err());
}

#[test]
fn test_emit_notifications_multiple_buffers() {
    let session = crate::session::Session::new(SessionId::new("multibuf-test"));
    let buf1 = reovim_kernel::api::v1::BufferId::from_raw(10);
    let buf2 = reovim_kernel::api::v1::BufferId::from_raw(20);

    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buf1);
    changes.record_buffer_modified(buf2);

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        0,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    // Should receive notifications for both buffers
    let mut received = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        received.push(notif);
    }
    assert!(!received.is_empty());
}

#[test]
fn test_resolve_to_command_context_large_count() {
    let ctx = ResolveContext {
        count: Some(999_999),
        register: None,
        ..ResolveContext::default()
    };
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    assert_eq!(cmd_ctx.count(), Some(999_999));
}

#[test]
fn test_resolve_to_command_context_special_register() {
    let ctx = ResolveContext {
        count: None,
        register: Some('*'),
        ..ResolveContext::default()
    };
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    assert_eq!(cmd_ctx.register(), Some('*'));
}

#[tokio::test]
async fn test_apply_mode_transition_for_follower_client() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("follower-trans-test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    // Set follower relation
    let _ = session.clients().set_client_relation(
        follower_id,
        Some(crate::session::ClientRelation::Following { target: owner_id }),
    );

    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let transition = ModeTransition::Push {
        mode: insert_mode,
        context: TransitionContext::new(),
    };

    // Follower should not be able to transition modes
    InputServiceImpl::apply_mode_transition_for_client(&session, follower_id, transition).await;

    // Follower client does not have independent mode tracking
    // so client_current_mode returns None - this is expected behavior
    assert!(session.client_current_mode(follower_id).is_none());
}

#[test]
fn test_handle_pop_result_execute_command_empty_args() {
    let session = crate::session::Session::new(SessionId::new("pop-noargs-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "noop",
    );

    // Execute with no args
    InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args: HashMap::new(),
        },
    );
}

#[test]
fn test_handle_pop_result_data_with_multiple_values() {
    let session = crate::session::Session::new(SessionId::new("pop-dataval-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let mut values = HashMap::new();
    values.insert(
        "mode".to_string(),
        reovim_subsys_command_types::ArgValue::String("visual".to_string()),
    );
    values.insert("count".to_string(), reovim_subsys_command_types::ArgValue::Count(10));
    values.insert("register".to_string(), reovim_subsys_command_types::ArgValue::Register('a'));

    InputServiceImpl::handle_pop_result_for_client(&session, client_id, PopResult::Data { values });
}

#[tokio::test]
async fn test_send_keys_empty_key_sequence() {
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(
        SendKeysRequest {
            keys: String::new(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;

    // Empty key sequence parses to None, which is invalid
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[test]
fn test_emit_notifications_with_affected_buffers() {
    let session = crate::session::Session::new(SessionId::new("affected-buf-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let buf1 = reovim_kernel::api::v1::BufferId::from_raw(1);
    let buf2 = reovim_kernel::api::v1::BufferId::from_raw(2);

    // Set up windows displaying the buffers for this client
    session.clients().update_client_state(client_id, |state| {
        let window = reovim_driver_text_session::Window::with_buffer(buf1);
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    let mut changes = StateChanges::new();
    changes.affected_buffers.push(buf1);
    changes.affected_buffers.push(buf2);
    changes.cursor_moved = true;

    let mut rx = session.subscribe_notifications();
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        client_id.as_usize() as u64,
        &BridgeRegistry::new(),
        &test_cache(),
    );

    // Should emit cursor_moved notifications
    let received = rx.try_recv();
    assert!(received.is_ok());
}

#[test]
fn test_input_service_impl_new_const() {
    // Test that new() is const
    let registry = test_registry();
    let session_id = SessionId::new("const-test");
    let _service = InputServiceImpl::new(registry, session_id, Arc::new(BridgeRegistry::new()));
}

#[tokio::test]
async fn test_apply_mode_transition_pop_multiple_times() {
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = crate::session::Session::new(SessionId::new("pop-multi-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push two modes
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let visual_mode = ModeId::new(ModuleId::new("test"), "visual");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
        state.mode_stack.push(visual_mode);
    });

    // Pop once
    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Pop { result: None },
    )
    .await;

    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "insert");

    // Pop again
    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Pop { result: None },
    )
    .await;

    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

#[test]
fn test_resolve_to_command_context_zero_count() {
    let ctx = ResolveContext {
        count: Some(0),
        register: None,
        ..ResolveContext::default()
    };
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    assert_eq!(cmd_ctx.count(), Some(0));
}

#[tokio::test]
#[cfg_attr(coverage_nightly, coverage(off))]
#[ignore = "Per-client state (#471): Requires resolver registration; panics without modules"]
async fn test_send_keys_independent_client() {
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Set client as Independent (default)
    let _ = session.clients().set_client_relation(client_id, None);

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(
        SendKeysRequest {
            keys: "<Esc>".to_string(),
        },
        client_id,
    );

    // Independent clients should not be rejected (will fail at resolver stage)
    let _response = service.send_keys(request).await;
    // Will panic without resolver, but that's expected behavior per #471
}

#[test]
fn test_handle_pop_result_for_nonexistent_client() {
    let session = crate::session::Session::new(SessionId::new("nonex-pop-test"));
    let nonexistent_id = ClientId::new(999);

    // Should not panic for nonexistent client
    InputServiceImpl::handle_pop_result_for_client(&session, nonexistent_id, PopResult::Cancelled);
}

#[tokio::test]
async fn test_handle_resolve_result_execute_with_changes() {
    use reovim_driver_text_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("exec-changes-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('x'));

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "test-cmd",
    );

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::Execute(cmd_id, ResolveContext::default()),
        &key,
        client_id,
    )
    .await;

    assert!(handled);
}

#[test]
fn test_emit_notifications_with_scrolled_windows() {
    let session = crate::session::Session::new(SessionId::new("scroll-test"));
    let window_id = reovim_kernel::api::v1::WindowId::new();
    let mut changes = StateChanges::new();
    changes.scroll_changed = true;
    changes.scrolled_windows.push(window_id);

    // Should not panic even without client windows
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        1,
        &BridgeRegistry::new(),
        &test_cache(),
    );
}

// =========================================================================
// Helper: Create session with a registered command
// =========================================================================

/// Create a session with a registered command that returns Success.
fn session_with_success_command(
    session_name: &str,
) -> (crate::session::Session, reovim_kernel::api::v1::CommandId) {
    session_with_result_command(session_name, CommandResult::Success)
}

/// Create a session with a registered command that returns Error.
fn session_with_error_command(
    session_name: &str,
    error_msg: &str,
) -> (crate::session::Session, reovim_kernel::api::v1::CommandId) {
    session_with_result_command(session_name, CommandResult::Error(error_msg.to_string()))
}

/// Create a session with a command that returns the given result.
fn session_with_result_command(
    session_name: &str,
    result: CommandResult,
) -> (crate::session::Session, reovim_kernel::api::v1::CommandId) {
    use {
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_text_session::SessionRuntime,
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    struct ConfiguredCmd {
        id: CommandId,
        result: CommandResult,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for ConfiguredCmd {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            "test command"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &[]
        }
    }

    impl CommandHandler for ConfiguredCmd {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            self.result.clone()
        }
    }

    let cmd_id = CommandId::new(ModuleId::new("test"), "test-configured-cmd");
    let session = crate::session::Session::new(SessionId::new(session_name));

    session.with_state_mut_sync(|state| {
        state.command_registry.register(Arc::new(ConfiguredCmd {
            id: cmd_id.clone(),
            result,
        }));
    });

    (session, cmd_id)
}

/// Create a simple resolver for the default `default/normal` mode that
/// returns `ResolveResult::Completed` for any key.
fn session_with_resolver(session_name: &str) -> crate::session::Session {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput,
        },
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    struct CompletedResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for CompletedResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    let session = crate::session::Session::new(SessionId::new(session_name));

    let mode = ModeId::new(ModuleId::new("default"), "normal");
    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(CompletedResolver { mode });
    });

    session
}

// =========================================================================
// Tests: Execute path with registered command (covers line 318)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_execute_with_registered_command() {
    use reovim_driver_text_input::KeyCode;

    let (session, cmd_id) = session_with_success_command("exec-reg-test");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('x'));

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::Execute(cmd_id, ResolveContext::default()),
        &key,
        client_id,
    )
    .await;

    assert!(handled);
}

// =========================================================================
// Tests: Execute with mode change during command (covers lines 327, 333)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_execute_mode_changes_during_command() {
    use {
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_text_input::{KeyCode, TransitionContext},
        reovim_driver_text_session::{SessionRuntime, api::ModeApi},
        reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
    };

    struct ModePushCmd {
        target_mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for ModePushCmd {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "push-mode-cmd")
        }
        fn description(&self) -> &'static str {
            "push mode"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &[]
        }
    }

    impl CommandHandler for ModePushCmd {
        fn execute(
            &self,
            runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            runtime.push_mode(self.target_mode.clone(), TransitionContext::new());
            CommandResult::Success
        }
    }

    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let push_cmd = ModePushCmd {
        target_mode: insert_mode,
    };
    let push_cmd_id = push_cmd.id();

    let session = crate::session::Session::new(SessionId::new("exec-modechange-test"));
    session.with_state_mut_sync(|state| {
        state.command_registry.register(Arc::new(push_cmd));
    });
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('i'));

    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::Execute(push_cmd_id, ResolveContext::default()),
        &key,
        client_id,
    )
    .await;

    assert!(handled);
    assert!(changes.mode_changed);
}

// =========================================================================
// Tests: InsertChar with buffer but no client window (covers early-return)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_insert_char_no_client_window() {
    use reovim_driver_text_input::{InputTarget, KeyCode};

    let session = crate::session::Session::new(SessionId::new("insertchar-buf-test"));

    // Create a buffer but add_client does not assign a window to it,
    // so insert_char_for_client returns None (no active window for client).
    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('x'));

    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InsertChar {
            char: 'x',
            target: InputTarget::Buffer,
        },
        &key,
        client_id,
    )
    .await;

    // Handled is true (input was consumed) but buffer not modified
    // because the client has no active window assigned.
    assert!(handled);
    assert!(!changes.buffer_modified);
}

#[tokio::test]
async fn test_handle_resolve_result_insert_newline_no_client_window() {
    use reovim_driver_text_input::{InputTarget, KeyCode};

    let session = crate::session::Session::new(SessionId::new("insert-newline-test"));

    session.with_state_mut_sync(|state| {
        state.create_buffer("line1\nline2");
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Enter);

    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InsertChar {
            char: '\n',
            target: InputTarget::Buffer,
        },
        &key,
        client_id,
    )
    .await;

    // Same as above: handled but no buffer modification without active window.
    assert!(handled);
    assert!(!changes.buffer_modified);
}

// =========================================================================
// Tests: handle_pop_result error path (covers lines 507, 511-517)
// =========================================================================

#[test]
fn test_handle_pop_result_execute_command_returns_error() {
    let (session, cmd_id) = session_with_error_command("pop-error-test", "test error message");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args: HashMap::new(),
        },
    );

    let dump = session.dump_client_ring_buffer(client_id);
    assert!(dump.is_some());
    let dump_str = dump.unwrap();
    assert!(
        dump_str.contains("COMMAND_FAILED"),
        "Ring buffer should contain COMMAND_FAILED entry, got: {dump_str}"
    );
}

#[test]
fn test_handle_pop_result_execute_error_with_args() {
    let (session, cmd_id) =
        session_with_error_command("pop-err-args-test", "arg processing failed");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let mut args = HashMap::new();
    args.insert("count".to_string(), reovim_subsys_command_types::ArgValue::Count(5));
    args.insert(
        "description".to_string(),
        reovim_subsys_command_types::ArgValue::String("test".to_string()),
    );

    InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args,
        },
    );

    let dump = session.dump_client_ring_buffer(client_id).unwrap();
    assert!(dump.contains("COMMAND_FAILED"));
    assert!(dump.contains("arg processing failed"));
}

// =========================================================================
// Tests: InjectKeys with non-empty keys (covers lines 390-410)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_inject_keys_with_resolver() {
    use reovim_driver_text_input::KeyCode;

    let session = session_with_resolver("inject-resolver-test");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('q'));

    let injected_keys = vec![
        reovim_driver_text_input::KeyEvent::new(KeyCode::Char('a')),
        reovim_driver_text_input::KeyEvent::new(KeyCode::Char('b')),
    ];

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InjectKeys {
            keys: injected_keys,
            exit_macro_playback: false,
        },
        &key,
        client_id,
    )
    .await;

    assert!(handled);
}

#[tokio::test]
async fn test_handle_resolve_result_inject_keys_nested_inject_skipped() {
    use {
        reovim_driver_text_input::{
            KeyCode, KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput,
        },
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    struct InjectResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for InjectResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::InjectKeys {
                keys: vec![DriverKeyEvent::new(KeyCode::Char('z'))],
                exit_macro_playback: false,
            }
        }
    }

    let session = crate::session::Session::new(SessionId::new("inject-nested-test"));
    let mode = ModeId::new(ModuleId::new("default"), "normal");
    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(InjectResolver { mode });
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('q'));

    let injected_keys = vec![reovim_driver_text_input::KeyEvent::new(KeyCode::Char('a'))];

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InjectKeys {
            keys: injected_keys,
            exit_macro_playback: false,
        },
        &key,
        client_id,
    )
    .await;

    assert!(handled);
}

// =========================================================================
// Tests: Full send_keys flow with resolver (covers lines 131-133, 156, 190)
// =========================================================================

#[tokio::test]
async fn test_send_keys_full_flow_with_resolver() {
    let session = session_with_resolver("sendkeys-flow-test");
    let client_id = ClientId::new(1);

    session.with_state_mut_sync(|state| {
        state.create_buffer("test content");
    });

    session.add_client(client_id);

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("sendkeys-flow-test"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
    assert_eq!(resp.status, i32::from(KeyStatus::Executed));
}

#[tokio::test]
async fn test_send_keys_multiple_keys_with_resolver() {
    let session = session_with_resolver("sendkeys-multi-test");
    let client_id = ClientId::new(1);

    session.with_state_mut_sync(|state| {
        state.create_buffer("content");
    });

    session.add_client(client_id);

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("sendkeys-multi-test"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "abc".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

#[tokio::test]
async fn test_send_keys_flow_without_active_buffer() {
    let session = session_with_resolver("sendkeys-nobuf-test");
    let client_id = ClientId::new(1);

    // Do NOT create a buffer - test the path where active_buffer() returns None
    session.add_client(client_id);

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("sendkeys-nobuf-test"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

// =========================================================================
// Coverage: InsertChar defensive no-op path
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_insert_char_buffer_modified() {
    use reovim_driver_text_input::{InputTarget, KeyCode};

    // Need a real BufferManager (not StubBufferManager) so buffers are actually stored
    let kernel = {
        let default = reovim_kernel::api::v1::KernelContext::default();
        default
            .services
            .register(std::sync::Arc::new(reovim_driver_text_buffer::TextBufferRegistry::new()));
        reovim_kernel::api::v1::KernelContext {
            buffers: std::sync::Arc::new(reovim_driver_text_buffer::TestBufferManager::new()),
            ..default
        }
    };
    let state = crate::session::SessionState::with_kernel(kernel);
    let session =
        crate::session::Session::from_state(SessionId::new("insertchar-modified-test"), state);

    // Create buffer, then add client so client gets a window for the buffer
    session
        .with_state_mut(|state| {
            state.create_buffer("hello");
        })
        .await;

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('y'));
    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InsertChar {
            char: 'y',
            target: InputTarget::Buffer,
        },
        &key,
        client_id,
    )
    .await;

    // InsertChar reaching the server dispatch is now a defensive no-op:
    // resolvers must handle insertion via SessionApi::resolve_with_session.
    // The result is handled=true (input consumed) but no buffer mutation.
    assert!(handled);
    assert!(
        !changes.buffer_modified,
        "InsertChar at server dispatch must not modify the buffer"
    );
}

// =========================================================================
// Coverage: Execute with operator completion (lines 309, 312-313)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_execute_with_operator_completion() {
    use {
        reovim_driver_text_input::{
            KeyCode, KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ModeTransition,
            ResolveInput, TransitionContext,
        },
        reovim_driver_text_session::{ExtensionMap, api::SessionApiDyn},
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    // Resolver that returns a mode transition on on_command_complete
    struct CompletionResolver {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for CompletionResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }

        fn on_command_complete(
            &self,
            _session: &mut dyn SessionApiDyn,
            _shared_extensions: &mut ExtensionMap,
            _client_extensions: &mut ExtensionMap,
        ) -> Option<ModeTransition> {
            Some(ModeTransition::Pop { result: None })
        }
    }

    // Use session_with_success_command to get a registered command
    let (session, cmd_id) = session_with_success_command("exec-operator-complete-test");

    // Replace the default resolver with our CompletionResolver
    let mode = ModeId::new(ModuleId::new("default"), "normal");
    session.with_state_mut_sync(|state| {
        state
            .resolver_registry
            .register(CompletionResolver { mode: mode.clone() });
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push an extra mode so Pop has something to pop
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Push {
            mode: insert_mode,
            context: TransitionContext::new(),
        },
    )
    .await;

    // Now pop back to normal mode, where our CompletionResolver lives
    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Pop { result: None },
    )
    .await;

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('d'));

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::Execute(cmd_id, reovim_driver_text_input::ResolveContext::new()),
        &key,
        client_id,
    )
    .await;

    assert!(handled);
}

// =========================================================================
// Coverage: tracing::debug closure in send_keys (line 156)
// =========================================================================

#[tokio::test]
async fn test_send_keys_with_debug_tracing_covers_closure() {
    // Set up DEBUG subscriber so tracing::debug! closures execute
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let session = session_with_resolver("sendkeys-tracing-test");
    let client_id = ClientId::new(1);

    session.with_state_mut_sync(|state| {
        state.create_buffer("test content");
    });

    session.add_client(client_id);

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("sendkeys-tracing-test"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;
    assert!(response.is_ok());
}

// =========================================================================
// Coverage: InjectKeys where resolve returns None (line 410)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_inject_keys_no_resolver() {
    use reovim_driver_text_input::KeyCode;

    // Session WITHOUT any resolver registered - resolve_key_for_client returns None
    let session = crate::session::Session::new(SessionId::new("inject-noresolver-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('q'));

    let injected_keys = vec![
        reovim_driver_text_input::KeyEvent::new(KeyCode::Char('a')),
        reovim_driver_text_input::KeyEvent::new(KeyCode::Char('b')),
    ];

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::InjectKeys {
            keys: injected_keys,
            exit_macro_playback: false,
        },
        &key,
        client_id,
    )
    .await;

    assert!(handled);
}

// =========================================================================
// ensure_selection_change_recorded tests
// =========================================================================

#[test]
fn test_ensure_selection_change_cursor_moved_with_selection() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    changes.record_cursor_move(buffer_id);
    assert!(!changes.selection_changed);

    // Window with selection
    let mut window = reovim_driver_text_session::Window::with_buffer(buffer_id);
    window.selection = Some(reovim_driver_text_session::api::Selection::character(
        reovim_driver_text_buffer::Position::new(0, 0),
        reovim_driver_text_buffer::Position::new(0, 5),
    ));
    let windows = reovim_driver_text_session::WindowLayout::single(window);

    InputServiceImpl::ensure_selection_change_recorded(&mut changes, &windows, Some(buffer_id));

    assert!(changes.selection_changed);
}

#[test]
fn test_ensure_selection_change_already_recorded_is_noop() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    changes.record_cursor_move(buffer_id);
    changes.selection_changed = true;

    let mut window = reovim_driver_text_session::Window::with_buffer(buffer_id);
    window.selection = Some(reovim_driver_text_session::api::Selection::character(
        reovim_driver_text_buffer::Position::new(0, 0),
        reovim_driver_text_buffer::Position::new(0, 5),
    ));
    let windows = reovim_driver_text_session::WindowLayout::single(window);

    // Already recorded — should not change anything
    InputServiceImpl::ensure_selection_change_recorded(&mut changes, &windows, Some(buffer_id));

    assert!(changes.selection_changed);
}

#[test]
fn test_ensure_selection_change_no_selection_is_noop() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    changes.record_cursor_move(buffer_id);

    // Window without selection
    let window = reovim_driver_text_session::Window::with_buffer(buffer_id);
    let windows = reovim_driver_text_session::WindowLayout::single(window);

    InputServiceImpl::ensure_selection_change_recorded(&mut changes, &windows, Some(buffer_id));

    assert!(!changes.selection_changed);
}

#[test]
fn test_ensure_selection_change_no_cursor_moved_is_noop() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    // cursor_moved is false

    let mut window = reovim_driver_text_session::Window::with_buffer(buffer_id);
    window.selection = Some(reovim_driver_text_session::api::Selection::character(
        reovim_driver_text_buffer::Position::new(0, 0),
        reovim_driver_text_buffer::Position::new(0, 5),
    ));
    let windows = reovim_driver_text_session::WindowLayout::single(window);

    InputServiceImpl::ensure_selection_change_recorded(&mut changes, &windows, Some(buffer_id));

    assert!(!changes.selection_changed);
}

// ========================================================================
// Generic bridge helper tests (#468)
// ========================================================================

/// Minimal bridge for testing generic bridge detection.
struct TestBridge {
    kind_str: &'static str,
    scope: reovim_driver_text_session::bridges::ExtensionScope,
}

impl TestBridge {
    const fn client(kind: &'static str) -> Self {
        Self {
            kind_str: kind,
            scope: reovim_driver_text_session::bridges::ExtensionScope::Client,
        }
    }
    const fn shared(kind: &'static str) -> Self {
        Self {
            kind_str: kind,
            scope: reovim_driver_text_session::bridges::ExtensionScope::Shared,
        }
    }
}

impl reovim_driver_text_session::bridges::ExtensionStateBridge for TestBridge {
    fn kind(&self) -> &'static str {
        self.kind_str
    }
    fn scope(&self) -> reovim_driver_text_session::bridges::ExtensionScope {
        self.scope
    }
    fn snapshot(
        &self,
        _extensions: &reovim_driver_text_session::ExtensionMap,
    ) -> Option<serde_json::Value> {
        None
    }
    fn is_active(&self, _extensions: &reovim_driver_text_session::ExtensionMap) -> bool {
        false
    }
}

#[test]
fn test_bridge_is_active_client_scope_no_client() {
    let session = Session::new(SessionId::new("bridge-test"));
    let bridge = TestBridge::client("test");
    // Client 99 doesn't exist — should return false (unwrap_or(false))
    let result = InputServiceImpl::bridge_is_active(&bridge, &session, ClientId::new(99));
    assert!(!result);
}

#[test]
fn test_bridge_is_active_client_scope_with_client() {
    let session = Session::new(SessionId::new("bridge-test-2"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let bridge = TestBridge::client("test");
    // TestBridge always returns false from is_active
    let result = InputServiceImpl::bridge_is_active(&bridge, &session, client_id);
    assert!(!result);
}

#[test]
fn test_bridge_is_active_shared_scope() {
    let session = Session::new(SessionId::new("bridge-test-3"));
    let bridge = TestBridge::shared("test");
    // TestBridge always returns false from is_active (shared scope)
    let result = InputServiceImpl::bridge_is_active(&bridge, &session, ClientId::new(1));
    assert!(!result);
}

#[test]
fn test_snapshot_bridge_states_empty_registry() {
    let session = Session::new(SessionId::new("snap-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let bridges = BridgeRegistry::new();
    let states = InputServiceImpl::snapshot_bridge_states(&session, client_id, &bridges);
    assert!(states.is_empty());
}

#[test]
fn test_snapshot_bridge_states_with_bridges() {
    let session = Session::new(SessionId::new("snap-test-2"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge::client("alpha"));
    bridges.register(TestBridge::shared("beta"));

    let states = InputServiceImpl::snapshot_bridge_states(&session, client_id, &bridges);
    assert_eq!(states.len(), 2);
    // Both TestBridge impls return false from is_active
    for &(_, active) in &states {
        assert!(!active);
    }
}

#[test]
fn test_detect_bridge_changes_no_change() {
    let session = Session::new(SessionId::new("detect-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge::client("test"));

    // Before: inactive, After: inactive (TestBridge always false)
    let before = vec![("test", false)];
    let mut changes = StateChanges::new();
    InputServiceImpl::detect_bridge_changes(&session, client_id, &bridges, &before, &mut changes);
    // No toggle and not active → no change recorded
    assert!(!changes.extension_changed);
}

#[test]
fn test_detect_bridge_changes_was_active_now_inactive() {
    let session = Session::new(SessionId::new("detect-test-2"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge::client("test"));

    // Before: active, After: inactive (TestBridge returns false)
    // was_active != is_active → should record change
    let before = vec![("test", true)];
    let mut changes = StateChanges::new();
    InputServiceImpl::detect_bridge_changes(&session, client_id, &bridges, &before, &mut changes);
    assert!(changes.extension_changed);
    assert!(changes.extensions_updated.contains(&"test".into()));
}

#[test]
fn test_detect_bridge_changes_empty_before() {
    let session = Session::new(SessionId::new("detect-test-3"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let bridges = BridgeRegistry::new();
    let before: Vec<(&str, bool)> = vec![];
    let mut changes = StateChanges::new();
    InputServiceImpl::detect_bridge_changes(&session, client_id, &bridges, &before, &mut changes);
    assert!(!changes.extension_changed);
}

// =========================================================================
// Snapshot deduplication tests (#691)
// =========================================================================

#[test]
fn test_snapshot_cache_starts_empty() {
    let cache = test_cache();
    assert!(cache.lock().is_empty());
}

#[test]
fn test_snapshot_cache_dedup_with_real_data() {
    let cache = test_cache();

    // Simulate: insert a cached entry, then check dedup logic
    {
        let mut c = cache.lock();
        c.insert(("test-ext".to_string(), 1), r#"{"active":true,"items":[]}"#.to_string());
    }

    // Same data should be detected as duplicate
    let c = cache.lock();
    let key = ("test-ext".to_string(), 1u64);
    let prev = c.get(&key).unwrap();
    assert_eq!(prev, r#"{"active":true,"items":[]}"#);
    // Different data should NOT match
    assert_ne!(prev, r#"{"active":true,"items":["new"]}"#);
    drop(c);
}

#[test]
fn test_snapshot_cache_different_client_ids_are_independent() {
    let cache = test_cache();

    {
        let mut c = cache.lock();
        c.insert(("ext".to_string(), 1), "data-1".to_string());
        c.insert(("ext".to_string(), 2), "data-2".to_string());
    }

    let c = cache.lock();
    assert_eq!(c.get(&("ext".to_string(), 1)).unwrap(), "data-1");
    assert_eq!(c.get(&("ext".to_string(), 2)).unwrap(), "data-2");
    drop(c);
}

// =========================================================================
// Codec index notification routing tests (#740 D.5)
// =========================================================================

#[test]
fn test_notify_codec_indices_uses_decoded_route_when_possible() {
    let buffer_id = BufferId::from_raw(1);
    let (session, calls) = make_codec_session("text/codec-route-1", b"D");
    record_codec_buffer(&session, buffer_id, b"abc", "text/codec-route-1");

    let mut changes = StateChanges::new();
    changes.record_buffer_modified_with_text_edit(
        buffer_id,
        reovim_driver_codec::TextBufferModified {
            buffer_id,
            edit: reovim_driver_codec::TextEdit::insert(
                reovim_driver_codec::TextPosition::new(0, 0),
                "x".to_string(),
            ),
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 1,
        },
    );

    InputServiceImpl::notify_codec_indices(&session, &changes);

    let bytes = session.with_state_sync(|state| {
        state
            .app
            .extensions
            .get::<reovim_driver_codec::CodecSessionState>()
            .and_then(|codec_state| codec_state.bytes(buffer_id))
    });
    assert_eq!(bytes, Some(b"Dabc".to_vec()));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

// =========================================================================
// Coverage: RuntimeSignal::Quit paths in handle_resolve_result (L789-800)
// =========================================================================

/// Create a session with a command that emits `RuntimeSignal::Quit` and
/// returns the specified result, for use in `handle_resolve_result` tests.
fn session_with_execute_quit_signal_command(
    session_name: &str,
    result: CommandResult,
) -> (crate::session::Session, reovim_kernel::api::v1::CommandId) {
    use {
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_text_session::SessionRuntime,
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    struct ExecuteQuitCmd {
        id: CommandId,
        result: CommandResult,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for ExecuteQuitCmd {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            "execute quit command"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &[]
        }
    }

    impl CommandHandler for ExecuteQuitCmd {
        fn execute(
            &self,
            runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            use reovim_subsys_command_types::RuntimeSignal;
            runtime.signal(RuntimeSignal::Quit);
            self.result.clone()
        }
    }

    let cmd_id = CommandId::new(ModuleId::new("test"), "execute-quit-cmd");
    let session = crate::session::Session::new(SessionId::new(session_name));
    session.with_state_mut_sync(|state| {
        state.command_registry.register(Arc::new(ExecuteQuitCmd {
            id: cmd_id.clone(),
            result,
        }));
    });
    (session, cmd_id)
}

#[tokio::test]
async fn test_handle_resolve_result_execute_quit_signal_success() {
    // Covers the Quit signal path in handle_resolve_result (L789-800).
    use reovim_driver_text_input::KeyCode;

    let (session, cmd_id) =
        session_with_execute_quit_signal_command("exec-quit-success-test", CommandResult::Success);
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    let key = reovim_driver_text_input::KeyEvent::new(KeyCode::Char('q'));
    let (handled, changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::Execute(cmd_id, ResolveContext::default()),
        &key,
        client_id,
    )
    .await;

    assert!(handled);
    assert!(changes.should_quit, "Quit signal should set should_quit");
}

#[test]
fn test_notify_codec_indices_uses_byte_edits_for_full_replace() {
    let buffer_id = BufferId::from_raw(2);
    let (session, calls) = make_codec_session("text/codec-route-2", b"D");
    record_codec_buffer(&session, buffer_id, b"abc", "text/codec-route-2");

    // FullReplace has no TextEdit equivalent, so text_buffer_edits is empty.
    // notify_codec_indices falls through to byte_edits.
    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buffer_id);
    changes.record_byte_edit(buffer_id, ByteEdit::replace(1, b"b", b"z"));

    InputServiceImpl::notify_codec_indices(&session, &changes);

    let bytes = session.with_state_sync(|state| {
        state
            .app
            .extensions
            .get::<reovim_driver_codec::CodecSessionState>()
            .and_then(|codec_state| codec_state.bytes(buffer_id))
    });
    assert_eq!(bytes, Some(b"azc".to_vec()));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn test_notify_codec_indices_skips_byte_updates_when_decoded_edit_already_applied() {
    let buffer_id = BufferId::from_raw(3);
    let (session, calls) = make_codec_session("text/codec-route-3", b"D");
    record_codec_buffer(&session, buffer_id, b"abc", "text/codec-route-3");

    let mut changes = StateChanges::new();
    changes.record_buffer_modified_with_text_edit(
        buffer_id,
        reovim_driver_codec::TextBufferModified {
            buffer_id,
            edit: reovim_driver_codec::TextEdit::insert(
                reovim_driver_codec::TextPosition::new(0, 0),
                "x".to_string(),
            ),
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 1,
        },
    );
    changes.record_byte_edit(buffer_id, ByteEdit::insert(0, b"X"));

    InputServiceImpl::notify_codec_indices(&session, &changes);

    let bytes = session.with_state_sync(|state| {
        state
            .app
            .extensions
            .get::<reovim_driver_codec::CodecSessionState>()
            .and_then(|codec_state| codec_state.bytes(buffer_id))
    });
    assert_eq!(bytes, Some(b"Dabc".to_vec()));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// When there is no `CodecSessionState` in the session extensions, the
/// `if let Some(codec_state)` at L639 is false and `notify_codec_indices`
/// returns immediately. This exercises the false-branch closing `}` at L664.
#[test]
fn test_notify_codec_indices_no_codec_state() {
    // Plain session with no CodecSessionState registered.
    let session = Arc::new(Session::new(SessionId::new("codec-no-state-test")));

    let buffer_id = BufferId::from_raw(42);
    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buffer_id);
    changes.record_byte_edit(buffer_id, reovim_kernel::api::v1::ByteEdit::insert(0, b"x"));

    // Should not panic and simply skip the inner block.
    InputServiceImpl::notify_codec_indices(&session, &changes);
}

// =========================================================================
// Coverage: RuntimeSignal::Quit in handle_pop_result_for_client
// (L1065-1069: error path, L1075-1079: success path)
// =========================================================================

/// Create a session with a command that emits `RuntimeSignal::Quit` and
/// returns the specified result.
fn session_with_quit_signal_command(
    session_name: &str,
    result: CommandResult,
) -> (crate::session::Session, reovim_kernel::api::v1::CommandId) {
    use {
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_text_session::SessionRuntime,
        reovim_kernel::api::v1::{CommandId, ModuleId},
        reovim_subsys_command_types::RuntimeSignal,
    };

    struct QuitSignalCmd {
        id: CommandId,
        result: CommandResult,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for QuitSignalCmd {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            "quit signal command"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &[]
        }
    }

    impl CommandHandler for QuitSignalCmd {
        fn execute(
            &self,
            runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            runtime.signal(RuntimeSignal::Quit);
            self.result.clone()
        }
    }

    let cmd_id = CommandId::new(ModuleId::new("test"), "quit-signal-cmd");
    let session = crate::session::Session::new(SessionId::new(session_name));
    session.with_state_mut_sync(|state| {
        state.command_registry.register(Arc::new(QuitSignalCmd {
            id: cmd_id.clone(),
            result,
        }));
    });
    (session, cmd_id)
}

#[test]
fn test_handle_pop_result_execute_command_success_with_quit_signal() {
    // Covers lines 1072-1080: success path with RuntimeSignal::Quit.
    let (session, cmd_id) =
        session_with_quit_signal_command("pop-quit-success-test", CommandResult::Success);
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let changes = InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args: HashMap::new(),
        },
    );

    // should_quit is set from record_quit_requested().
    assert!(changes.should_quit, "Quit signal should set should_quit");
}

#[test]
fn test_handle_pop_result_execute_command_error_with_quit_signal() {
    // Covers lines 1053-1069: error path with RuntimeSignal::Quit.
    let (session, cmd_id) = session_with_quit_signal_command(
        "pop-quit-error-test",
        CommandResult::Error("some error".to_string()),
    );
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let changes = InputServiceImpl::handle_pop_result_for_client(
        &session,
        client_id,
        PopResult::ExecuteCommand {
            command: cmd_id,
            args: HashMap::new(),
        },
    );

    // should_quit is set from record_quit_requested() in the error path too.
    assert!(changes.should_quit, "Quit signal in error path should set should_quit");
}

// =========================================================================
// notify_bridges_mode_changed direct tests (L417-433)
// =========================================================================

/// `notify_bridges_mode_changed` should call `on_mode_changed` on every
/// `Client`-scoped bridge and skip `Shared`-scoped bridges.
#[test]
fn test_notify_bridges_mode_changed_calls_client_bridges() {
    let session = Session::new(SessionId::new("notify-mode-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge::client("mode-bridge"));
    bridges.register(TestBridge::shared("shared-bridge"));

    // Should not panic — exercises the iteration + scope filter body.
    InputServiceImpl::notify_bridges_mode_changed(
        &session, client_id, &bridges, "normal", "insert",
    );
}

/// `notify_bridges_mode_changed` with no registered bridges — empty iteration.
#[test]
fn test_notify_bridges_mode_changed_empty_registry() {
    let session = Session::new(SessionId::new("notify-mode-empty-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let bridges = BridgeRegistry::new();
    InputServiceImpl::notify_bridges_mode_changed(&session, client_id, &bridges, "a", "b");
}

// =========================================================================
// notify_bridges_cursor_moved direct tests (L439-468)
// =========================================================================

/// `notify_bridges_cursor_moved` with a client that has an active window —
/// exercises the full body including the cursor-position lookup.
#[test]
fn test_notify_bridges_cursor_moved_with_window() {
    let session = Session::new(SessionId::new("notify-cursor-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    session.clients().update_client_state(client_id, |state| {
        let window = reovim_driver_text_session::Window::with_buffer(buffer_id);
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge::client("cursor-bridge"));

    let mut changes = StateChanges::new();
    InputServiceImpl::notify_bridges_cursor_moved(&session, client_id, &bridges, &mut changes);
    // No assertions beyond "did not panic".
}

/// `notify_bridges_cursor_moved` with no active window — exercises the early
/// return branch.
#[test]
fn test_notify_bridges_cursor_moved_no_window() {
    let session = Session::new(SessionId::new("notify-cursor-nowin-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    // No windows configured — `active()` returns None → early return.

    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge::client("cursor-bridge"));

    let mut changes = StateChanges::new();
    InputServiceImpl::notify_bridges_cursor_moved(&session, client_id, &bridges, &mut changes);
}

// =========================================================================
// send_keys post-processing: active_buffer + viewport (L222-265)
// =========================================================================

/// Create a session with:
/// - A `CompletedResolver` (returns `ResolveResult::Completed` for any key)
/// - A real buffer manager
/// - A buffer created and registered as the client's active buffer
/// - A client with a window + viewport
fn session_with_resolver_and_active_buffer(
    name: &str,
) -> (crate::session::Session, reovim_kernel::api::v1::BufferId, ClientId) {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput,
        },
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    struct CompletedResolver {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for CompletedResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    let kernel = {
        use reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry};
        let services = Arc::new(ServiceRegistry::new());
        services.register(Arc::new(reovim_driver_text_buffer::TextBufferRegistry::new()));
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(reovim_driver_text_buffer::TestBufferManager::new()),
            Arc::new(OptionRegistry::new()),
            services,
        )
    };
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = crate::session::Session::from_state(SessionId::new(name), state);

    let mode = ModeId::new(ModuleId::new("default"), "normal");
    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(CompletedResolver { mode });
    });

    // Create a buffer
    let buffer_id = session.with_state_mut_sync(|state| state.create_buffer("content"));

    // Add client
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Set active_buffer and a window for the client
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buffer_id);
        let window = reovim_driver_text_session::Window::with_buffer(buffer_id);
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    (session, buffer_id, client_id)
}

/// `send_keys` with a resolver + active buffer covers the cursor-move and
/// viewport-scroll post-processing branches (L222-244).
#[tokio::test]
async fn test_send_keys_post_processing_with_active_buffer() {
    let (session, _buffer_id, client_id) =
        session_with_resolver_and_active_buffer("sendkeys-postproc-test");

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("sendkeys-postproc-test"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

/// `send_keys` with mode-change bridges covers `notify_bridges_mode_changed`
/// being called from the `send_keys` loop (L289-296).
#[tokio::test]
async fn test_send_keys_with_mode_change_notifies_bridges() {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput, TransitionContext,
        },
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    // A resolver that transitions mode on key press
    struct ModeChangeResolver {
        mode: ModeId,
        target: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for ModeChangeResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::ModeTransition(reovim_driver_text_input::ModeTransition::Push {
                mode: self.target.clone(),
                context: TransitionContext::new(),
            })
        }
    }

    // Also need a resolver for the target mode
    struct TargetResolver {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for TargetResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    let session = crate::session::Session::new(SessionId::new("sendkeys-modechange-test"));
    let normal_mode = ModeId::new(ModuleId::new("default"), "normal");
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");

    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(ModeChangeResolver {
            mode: normal_mode,
            target: insert_mode.clone(),
        });
        state
            .resolver_registry
            .register(TargetResolver { mode: insert_mode });
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge::client("mode-bridge"));

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("sendkeys-modechange-test"),
        Arc::new(bridges),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;
    assert!(response.is_ok());
}

// =========================================================================
// Coverage: notify_bridges_cursor_moved deactivation branch (L463,465)
// =========================================================================

/// A `SessionExtension` that carries a single boolean `active` flag.
/// Used by `DeactivatingBridge` to simulate a bridge that deactivates on cursor move.
#[derive(Default)]
struct DeactivatingBridgeState {
    active: bool,
}

impl reovim_driver_text_session::SessionExtension for DeactivatingBridgeState {
    fn create() -> Self {
        Self { active: true }
    }
}

/// A bridge that starts active and deactivates when `on_cursor_moved` is called.
///
/// This exercises the `was_active && !is_active` branch in
/// `notify_bridges_cursor_moved` (L462-464 in input.rs).
struct DeactivatingBridge;

impl reovim_driver_text_session::bridges::ExtensionStateBridge for DeactivatingBridge {
    fn kind(&self) -> &'static str {
        "deactivating-bridge"
    }

    fn scope(&self) -> reovim_driver_text_session::bridges::ExtensionScope {
        reovim_driver_text_session::bridges::ExtensionScope::Client
    }

    fn snapshot(
        &self,
        _extensions: &reovim_driver_text_session::ExtensionMap,
    ) -> Option<serde_json::Value> {
        None
    }

    fn is_active(&self, extensions: &reovim_driver_text_session::ExtensionMap) -> bool {
        extensions
            .get::<DeactivatingBridgeState>()
            .is_some_and(|s| s.active)
    }

    fn on_cursor_moved(
        &self,
        _line: usize,
        _col: usize,
        extensions: &mut reovim_driver_text_session::ExtensionMap,
    ) {
        // Deactivate on cursor move
        if let Some(state) = extensions.get_mut::<DeactivatingBridgeState>() {
            state.active = false;
        }
    }
}

/// `notify_bridges_cursor_moved` with a bridge that deactivates on cursor move.
///
/// Before `on_cursor_moved`: `is_active` returns true.
/// After `on_cursor_moved`:  `is_active` returns false.
///
/// This exercises L462-464: `if was_active && !is_active { changes.record_extension_change(...) }`.
#[test]
fn test_notify_bridges_cursor_moved_bridge_deactivates() {
    let session = Session::new(SessionId::new("cursor-deactivate-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Pre-seed the per-client extension with `active = true` so the bridge
    // finds it active before the cursor-moved hook runs.
    session.clients().update_client_state(client_id, |state| {
        state
            .extensions
            .get_or_insert::<DeactivatingBridgeState>()
            .active = true;
    });

    // Set up a window so notify_bridges_cursor_moved does not early-return.
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    session.clients().update_client_state(client_id, |state| {
        let window = reovim_driver_text_session::Window::with_buffer(buffer_id);
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    let mut bridges = BridgeRegistry::new();
    bridges.register(DeactivatingBridge);

    let mut changes = StateChanges::new();
    InputServiceImpl::notify_bridges_cursor_moved(&session, client_id, &bridges, &mut changes);

    // The bridge went from active -> inactive, so an extension change must be recorded.
    assert!(
        changes.extension_changed,
        "expected extension_changed after bridge deactivation"
    );
    assert!(
        changes
            .extensions_updated
            .contains(&"deactivating-bridge".into()),
        "expected 'deactivating-bridge' in extensions_updated"
    );
}

/// A bridge that is always active and never deactivates on cursor movement.
///
/// This exercises the `was_active && !is_active` MC/DC condition at L462
/// for the case where `was_active=true` AND `is_active=true` (condition false).
/// The outer `if scope == Client` is entered (covering L465) but the inner
/// `if was_active && !is_active` is false so L463 is skipped.
struct StayActiveBridgeState {
    active: bool,
}

impl reovim_driver_text_session::SessionExtension for StayActiveBridgeState {
    fn create() -> Self {
        Self { active: true }
    }
}

struct StayActiveBridge;

impl reovim_driver_text_session::bridges::ExtensionStateBridge for StayActiveBridge {
    fn kind(&self) -> &'static str {
        "stay-active-bridge"
    }

    fn scope(&self) -> reovim_driver_text_session::bridges::ExtensionScope {
        reovim_driver_text_session::bridges::ExtensionScope::Client
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn snapshot(
        &self,
        _extensions: &reovim_driver_text_session::ExtensionMap,
    ) -> Option<serde_json::Value> {
        None
    }

    fn is_active(&self, extensions: &reovim_driver_text_session::ExtensionMap) -> bool {
        extensions
            .get::<StayActiveBridgeState>()
            .is_some_and(|s| s.active)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn on_cursor_moved(
        &self,
        _line: usize,
        _col: usize,
        _extensions: &mut reovim_driver_text_session::ExtensionMap,
    ) {
        // Intentionally does nothing — bridge stays active.
    }
}

/// `notify_bridges_cursor_moved` with a bridge that stays active on cursor move.
///
/// Before `on_cursor_moved`: `is_active` returns true.
/// After `on_cursor_moved`:  `is_active` still returns true.
///
/// This exercises the MC/DC false-branch of `was_active && !is_active` (L462)
/// when `was_active=true` but `!is_active=false`, so no extension change is
/// recorded.  The outer `if scope == Client` body is still entered (L465
/// closing brace is hit).
#[test]
fn test_notify_bridges_cursor_moved_bridge_stays_active() {
    let session = Session::new(SessionId::new("cursor-stayactive-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Pre-seed the per-client extension with `active = true`.
    session.clients().update_client_state(client_id, |state| {
        state
            .extensions
            .get_or_insert::<StayActiveBridgeState>()
            .active = true;
    });

    // Set up a window so notify_bridges_cursor_moved does not early-return.
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    session.clients().update_client_state(client_id, |state| {
        let window = reovim_driver_text_session::Window::with_buffer(buffer_id);
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    let mut bridges = BridgeRegistry::new();
    bridges.register(StayActiveBridge);

    let mut changes = StateChanges::new();
    InputServiceImpl::notify_bridges_cursor_moved(&session, client_id, &bridges, &mut changes);

    // Bridge did NOT deactivate, so extension_changed must be false.
    assert!(
        !changes.extension_changed,
        "expected no extension change when bridge stays active"
    );
}

// =========================================================================
// Local test syntax driver for emit_syntax_updates tests
// =========================================================================

mod local_syntax_driver {
    use std::{ops::Range, sync::Arc};

    use reovim_driver_text_syntax::{Annotation, SyntaxDriver, SyntaxDriverFactory, SyntaxEdit};

    pub struct LocalTestDriver {
        language: String,
        parsed: bool,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl LocalTestDriver {
        pub fn new(language: &str) -> Self {
            Self {
                language: language.to_string(),
                parsed: false,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SyntaxDriver for LocalTestDriver {
        fn language(&self) -> &str {
            &self.language
        }

        fn parse(&mut self, _content: &str) {
            self.parsed = true;
        }

        fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {}

        fn highlights(&self, _byte_range: Range<usize>) -> Vec<Annotation> {
            vec![]
        }

        fn is_parsed(&self) -> bool {
            self.parsed
        }
    }

    pub struct LocalTestFactory;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SyntaxDriverFactory for LocalTestFactory {
        fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
            if language_id == "text" || language_id == "rust" {
                Some(Box::new(LocalTestDriver::new(language_id)))
            } else {
                None
            }
        }

        fn supported_languages(&self) -> Vec<&str> {
            vec!["text", "rust"]
        }

        fn supports(&self, language_id: &str) -> bool {
            language_id == "text" || language_id == "rust"
        }
    }

    pub fn local_test_factory() -> Arc<dyn SyntaxDriverFactory> {
        Arc::new(LocalTestFactory)
    }
}

// =========================================================================
// emit_syntax_updates: full function body (L555-627)
// =========================================================================

/// Helper: build a session with a real buffer manager, `TextBufferRegistry`, and
/// a local syntax factory so `emit_syntax_updates` can create drivers.
fn session_with_syntax_support(name: &str) -> (crate::session::Session, BufferId) {
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
    let session = crate::session::Session::from_state(SessionId::new(name), state);

    // Install syntax factory and language registry so ensure_driver_from_path
    // can detect "rust" from ".rs" paths and create a driver (covers L604-608, L622-623).
    {
        use reovim_driver_text_syntax::{DefaultLanguageRegistry, LanguageInfo};
        let registry = Arc::new(DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        ]));
        session.with_state_mut_sync(|state| {
            let syntax_state = state
                .app
                .extensions
                .get_or_insert::<crate::session::SyntaxSessionState>();
            syntax_state.set_factory(local_syntax_driver::local_test_factory());
            syntax_state.set_registry(registry);
        });
    }

    let buffer_id = session.with_state_mut_sync(|state| state.create_buffer("fn main() {}"));

    // Attach a .rs file path so ensure_driver_from_path creates a driver
    session.with_state_mut_sync(|state| {
        if let Some(buf_arc) = state.buffer(buffer_id) {
            buf_arc.write().set_file_path(Some("main.rs".to_string()));
        }
    });

    (session, buffer_id)
}

#[test]
fn test_emit_syntax_updates_with_modified_buffer_no_text_edit() {
    // Exercises emit_syntax_updates: buffer has file_path, driver is created,
    // full reparse (no text edit available) runs, build_token_update + broadcast.
    let (session, buffer_id) = session_with_syntax_support("syntax-update-noedit-test");

    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buffer_id);
    changes.modified_buffers.push(buffer_id);

    // Should not panic
    InputServiceImpl::emit_syntax_updates(&session, &changes);
}

#[test]
fn test_emit_syntax_updates_with_text_edit() {
    // Exercises the incremental-update branch (edit_info is Some).
    let (session, buffer_id) = session_with_syntax_support("syntax-update-edit-test");

    let mut changes = StateChanges::new();
    changes.record_buffer_modified_with_text_edit(
        buffer_id,
        reovim_driver_codec::TextBufferModified {
            buffer_id,
            edit: reovim_driver_codec::TextEdit::insert(
                reovim_driver_codec::TextPosition::new(0, 0),
                "x".to_string(),
            ),
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 1,
        },
    );
    changes.modified_buffers.push(buffer_id);

    InputServiceImpl::emit_syntax_updates(&session, &changes);
}

#[test]
fn test_emit_syntax_updates_deleted_buffers_cleanup() {
    // Exercises the buffers_deleted cleanup path (L561-568).
    let (session, buffer_id) = session_with_syntax_support("syntax-update-delete-test");

    // First create a driver entry for the buffer
    let mut setup_changes = StateChanges::new();
    setup_changes.record_buffer_modified(buffer_id);
    setup_changes.modified_buffers.push(buffer_id);
    InputServiceImpl::emit_syntax_updates(&session, &setup_changes);

    // Now delete it
    let mut changes = StateChanges::new();
    changes.buffers_deleted.push(buffer_id);

    InputServiceImpl::emit_syntax_updates(&session, &changes);
}

#[test]
fn test_emit_syntax_updates_empty_modified_buffers_returns_early() {
    // When modified_buffers is empty, the function returns early (L570-572).
    let (session, _) = session_with_syntax_support("syntax-update-empty-test");

    let mut changes = StateChanges::new();
    // modified_buffers is empty — covers the early-return branch
    changes.buffer_modified = true;

    InputServiceImpl::emit_syntax_updates(&session, &changes);
}

#[test]
fn test_emit_syntax_updates_missing_buffer_in_state() {
    // When buffer(buffer_id) returns None (buffer not in kernel), the inner
    // `continue` at L577-579 is exercised.
    let (session, _) = session_with_syntax_support("syntax-update-nobuf-test");

    let phantom_buffer_id = BufferId::from_raw(999);
    let mut changes = StateChanges::new();
    changes.record_buffer_modified(phantom_buffer_id);
    changes.modified_buffers.push(phantom_buffer_id);

    InputServiceImpl::emit_syntax_updates(&session, &changes);
}

/// Buffer exists but has no file path and no syntax driver installed.
/// `syntax.get_mut(buffer_id)` returns `None`, covering the `if let` false path.
#[test]
fn test_emit_syntax_updates_buffer_exists_but_no_driver() {
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
    let session =
        crate::session::Session::from_state(SessionId::new("syntax-no-driver-test"), state);

    // Create a buffer WITHOUT a file path — ensure_driver_from_path is skipped.
    // No syntax driver is installed — get_mut(buffer_id) returns None.
    let buffer_id = session.with_state_mut_sync(|state| state.create_buffer("hello"));

    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buffer_id);
    changes.modified_buffers.push(buffer_id);

    // Should not panic; the `if let Some(driver)` block is skipped.
    InputServiceImpl::emit_syntax_updates(&session, &changes);
}

#[test]
fn test_emit_syntax_updates_with_subscriber_receives_broadcast() {
    // Exercises the `stream.broadcast(&update)` path (L621-624): a subscriber
    // is registered so the Some(update) branch is entered.
    use crate::session::SyntaxStreamState;

    let (session, buffer_id) = session_with_syntax_support("syntax-update-broadcast-test");

    // Subscribe to the stream so broadcast actually sends
    session.with_state_mut_sync(|state| {
        let stream = state.app.extensions.get_or_insert::<SyntaxStreamState>();
        let _rx = stream.subscribe();
    });

    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buffer_id);
    changes.modified_buffers.push(buffer_id);

    InputServiceImpl::emit_syntax_updates(&session, &changes);
}

// =========================================================================
// text_edit_to_decoded_edit: Delete variant (L681-698)
// =========================================================================

#[test]
fn test_notify_codec_indices_delete_text_edit_single_line() {
    // Exercises text_edit_to_decoded_edit with TextEdit::Delete (no newline).
    let buffer_id = BufferId::from_raw(10);
    let (session, _calls) = make_codec_session("text/codec-delete-test", b"D");
    record_codec_buffer(&session, buffer_id, b"hello world", "text/codec-delete-test");

    let mut changes = StateChanges::new();
    changes.record_buffer_modified_with_text_edit(
        buffer_id,
        reovim_driver_codec::TextBufferModified {
            buffer_id,
            edit: reovim_driver_codec::TextEdit::delete(
                reovim_driver_codec::TextPosition::new(0, 0),
                "hel".to_string(),
            ),
            start_byte: 0,
            old_end_byte: 3,
            new_end_byte: 0,
        },
    );

    InputServiceImpl::notify_codec_indices(&session, &changes);
}

#[test]
fn test_notify_codec_indices_delete_text_edit_with_newline() {
    // Exercises the `ch == '\n'` branch inside text_edit_to_decoded_edit's
    // Delete computation (L686-689): deleted text spans a line boundary.
    let buffer_id = BufferId::from_raw(11);
    let (session, _calls) = make_codec_session("text/codec-delete-nl", b"D");
    record_codec_buffer(&session, buffer_id, b"line1\nline2", "text/codec-delete-nl");

    let mut changes = StateChanges::new();
    changes.record_buffer_modified_with_text_edit(
        buffer_id,
        reovim_driver_codec::TextBufferModified {
            buffer_id,
            edit: reovim_driver_codec::TextEdit::delete(
                reovim_driver_codec::TextPosition::new(0, 0),
                "line1\nli".to_string(),
            ),
            start_byte: 0,
            old_end_byte: 8,
            new_end_byte: 0,
        },
    );

    InputServiceImpl::notify_codec_indices(&session, &changes);
}

// =========================================================================
// resolve_to_command_context: uncovered metadata variants
// (L715-723: Bool, Char, Int, Uint, Position, Float, Range)
// =========================================================================

#[test]
fn test_resolve_to_command_context_bool_metadata() {
    let mut ctx = ResolveContext::default();
    ctx.metadata
        .insert("flag".to_string(), reovim_driver_text_input::ArgValue::Bool(true));
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    // Bool converts to ArgValue::Bool; bool_flag reads it back
    assert!(cmd_ctx.bool_flag("flag"));
}

#[test]
fn test_resolve_to_command_context_char_metadata() {
    let mut ctx = ResolveContext::default();
    ctx.metadata
        .insert("ch".to_string(), reovim_driver_text_input::ArgValue::Char('z'));
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    assert!(cmd_ctx.char("ch").is_some());
}

#[test]
fn test_resolve_to_command_context_int_metadata_positive() {
    let mut ctx = ResolveContext::default();
    // Int(5) → ArgValue::Count(5) inserted as key "count"
    ctx.metadata
        .insert("count".to_string(), reovim_driver_text_input::ArgValue::Int(5));
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    // Positive Int converts to ArgValue::Count; top-level count() reads the "count" key
    assert_eq!(cmd_ctx.count(), Some(5));
}

#[test]
fn test_resolve_to_command_context_int_metadata_negative() {
    // Negative Int: usize::try_from fails, branch produces None (no entry set).
    let mut ctx = ResolveContext::default();
    ctx.metadata
        .insert("neg".to_string(), reovim_driver_text_input::ArgValue::Int(-1));
    // Should not panic
    let _cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
}

#[test]
fn test_resolve_to_command_context_uint_metadata() {
    let mut ctx = ResolveContext::default();
    // Uint(42) → ArgValue::Count(42) stored at key "motion_count"
    ctx.metadata
        .insert("motion_count".to_string(), reovim_driver_text_input::ArgValue::Uint(42));
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    // The value is stored but only accessible via get() since key is not "count"
    // We just verify the conversion branch runs without panic.
    let _ = cmd_ctx;
}

#[test]
fn test_resolve_to_command_context_position_metadata() {
    let mut ctx = ResolveContext::default();
    ctx.metadata.insert(
        "cursor".to_string(),
        reovim_driver_text_input::ArgValue::Position(reovim_driver_text_buffer::Position::new(
            3, 7,
        )),
    );
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    // Position converts to ArgValue::Position(line, col); cursor_position reads back "cursor"
    let pos = cmd_ctx.cursor_position();
    assert!(pos.is_some());
    let (line, col) = pos.unwrap();
    assert_eq!(line, 3);
    assert_eq!(col, 7);
}

#[test]
fn test_resolve_to_command_context_float_metadata_skipped() {
    // Float is skipped (produces None), exercises L721-723 trace branch.
    let mut ctx = ResolveContext::default();
    ctx.metadata
        .insert("f".to_string(), reovim_driver_text_input::ArgValue::Float(1.5));
    let cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
    // "f" was not inserted because Float → None
    assert!(cmd_ctx.count().is_none());
}

#[test]
fn test_resolve_to_command_context_range_metadata_skipped() {
    // Range variant is also skipped (L721 `Range { .. }`).
    let mut ctx = ResolveContext::default();
    ctx.metadata.insert(
        "r".to_string(),
        reovim_driver_text_input::ArgValue::Range {
            start: reovim_driver_text_buffer::Position::new(0, 0),
            end: reovim_driver_text_buffer::Position::new(1, 5),
            linewise: false,
        },
    );
    let _cmd_ctx = InputServiceImpl::resolve_to_command_context(&ctx);
}

// =========================================================================
// emit_notifications dedup: suppress identical ExtensionUpdated (L518-526)
// =========================================================================

/// A bridge that is always active and returns a fixed JSON snapshot.
///
/// Used to trigger the `ExtensionUpdated` notification so the dedup
/// cache in `emit_notifications` can be exercised.
struct ConstantActiveBridge;

impl reovim_driver_text_session::bridges::ExtensionStateBridge for ConstantActiveBridge {
    fn kind(&self) -> &'static str {
        "constant-active"
    }

    fn scope(&self) -> reovim_driver_text_session::bridges::ExtensionScope {
        reovim_driver_text_session::bridges::ExtensionScope::Client
    }

    fn snapshot(
        &self,
        _extensions: &reovim_driver_text_session::ExtensionMap,
    ) -> Option<serde_json::Value> {
        Some(serde_json::json!({"active": true, "value": 42}))
    }

    fn is_active(&self, _extensions: &reovim_driver_text_session::ExtensionMap) -> bool {
        true
    }
}

#[test]
fn test_emit_notifications_dedup_suppresses_identical_extension_update() {
    // First emission: extension_changed is true, bridge returns Some JSON.
    // The notification is emitted and cached.
    // Second emission with the same JSON: it should be suppressed (L520-525).
    let session = Session::new(SessionId::new("dedup-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let mut bridges = BridgeRegistry::new();
    bridges.register(ConstantActiveBridge);

    let cache = test_cache();

    // Build changes with extension_changed so the ExtensionUpdated notification
    // is generated by build_notifications.
    let mut changes = StateChanges::new();
    changes.record_extension_change("constant-active".into());

    let mut rx = session.subscribe_notifications();

    // First call: notification is new → emitted and cached
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        client_id.as_usize() as u64,
        &bridges,
        &cache,
    );

    // Drain the first notification
    let first = rx.try_recv();
    assert!(first.is_ok(), "First emission should produce a notification");

    // Second call with identical state: same JSON → suppressed
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        client_id.as_usize() as u64,
        &bridges,
        &cache,
    );

    // The second call should not emit a new notification for this extension
    // (it may emit others, but the extension_updated one is suppressed).
    // Drain and verify no extension_updated notification was re-sent.
    let mut saw_extension_updated = false;
    while let Ok(notif) = rx.try_recv() {
        if notif.event_type == "extension_updated" {
            saw_extension_updated = true;
        }
    }
    assert!(
        !saw_extension_updated,
        "Identical ExtensionUpdated should be suppressed by dedup cache"
    );
}

#[test]
fn test_emit_notifications_dedup_allows_different_extension_data() {
    // Verify that after the cache stores data "A", a different payload "B"
    // on a second call IS emitted (not suppressed).
    let session = Session::new(SessionId::new("dedup-diff-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Manually prime the cache with stale data so the second emission differs.
    let cache = test_cache();
    {
        let mut c = cache.lock();
        c.insert(
            ("constant-active".to_string(), client_id.as_usize() as u64),
            r#"{"stale":true}"#.to_string(),
        );
    }

    let mut bridges = BridgeRegistry::new();
    bridges.register(ConstantActiveBridge);

    let mut changes = StateChanges::new();
    changes.record_extension_change("constant-active".into());

    let mut rx = session.subscribe_notifications();

    // ConstantActiveBridge returns `{"active":true,"value":42}` which differs
    // from the stale cache entry → should emit (not suppress).
    InputServiceImpl::emit_notifications(
        &session,
        &changes,
        client_id.as_usize() as u64,
        &bridges,
        &cache,
    );

    let mut saw_extension_updated = false;
    while let Ok(notif) = rx.try_recv() {
        if notif.event_type == "extension_updated" {
            saw_extension_updated = true;
        }
    }
    assert!(
        saw_extension_updated,
        "Different ExtensionUpdated payload should NOT be suppressed"
    );
}

// =========================================================================
// send_keys post-processing: viewport scroll (L235-243) and
// presence update (L277-285)
// =========================================================================

/// Build a `CompletedResolver` session that also triggers viewport scroll
/// by placing the cursor outside the viewport before `send_keys`.
///
/// Returns (session, `client_id`, `buffer_id`).
fn session_for_viewport_scroll(name: &str) -> (crate::session::Session, ClientId, BufferId) {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput,
        },
        reovim_kernel::api::v1::ModeId,
    };

    struct CompletedResolver {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for CompletedResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    let kernel = {
        use reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry};
        let services = Arc::new(ServiceRegistry::new());
        services.register(Arc::new(reovim_driver_text_buffer::TextBufferRegistry::new()));
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(reovim_driver_text_buffer::TestBufferManager::new()),
            Arc::new(OptionRegistry::new()),
            services,
        )
    };
    let state = crate::session::SessionState::with_kernel(kernel);
    let session = crate::session::Session::from_state(SessionId::new(name), state);

    let mode = reovim_kernel::api::v1::ModeId::new(
        reovim_kernel::api::v1::ModuleId::new("default"),
        "normal",
    );
    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(CompletedResolver { mode });
    });

    let buffer_id = session.with_state_mut_sync(|state| state.create_buffer("content"));

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buffer_id);
        let mut window = reovim_driver_text_session::Window::with_buffer(buffer_id);
        // Set cursor to line 30 — beyond the default 24-line viewport
        // so ensure_cursor_visible returns true, triggering scroll tracking.
        window.cursor.line = 30;
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    (session, client_id, buffer_id)
}

#[tokio::test]
async fn test_send_keys_viewport_scroll_triggered() {
    // Covers L235-243: cursor outside viewport causes ensure_cursor_visible
    // to return true, record_scroll_change is called.
    let (session, client_id, _buffer_id) = session_for_viewport_scroll("sendkeys-scroll-test");

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("sendkeys-scroll-test"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(resp.ok);
}

/// Resolver that executes a command which calls `runtime.focus_window()`,
/// causing `focus_changed = true` in `accumulated_changes`.
/// This triggers the presence update path (L277-285).
#[allow(clippy::too_many_lines)] // test setup is inherently verbose
#[tokio::test]
async fn test_send_keys_presence_update_on_focus_changed() {
    // Covers L277-285: when accumulated_changes.focus_changed is true,
    // the presence update path is taken.
    //
    // We achieve `focus_changed = true` by registering a command that calls
    // `runtime.focus_window(active_window)` — this uses WindowApi::focus_window.
    use {
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput,
        },
        reovim_driver_text_session::{SessionRuntime, api::WindowApi},
        reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
    };

    struct FocusWindowCmd {
        id: CommandId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for FocusWindowCmd {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            "focus window cmd"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &[]
        }
    }

    impl CommandHandler for FocusWindowCmd {
        fn execute(
            &self,
            runtime: &mut SessionRuntime<'_>,
            _args: &reovim_subsys_command_types::CommandContext,
        ) -> reovim_subsys_command_types::CommandResult {
            // Focus the active window to set focus_changed = true
            if let Some(win_id) = runtime.active_window() {
                let _ = runtime.focus_window(win_id);
            }
            reovim_subsys_command_types::CommandResult::Success
        }
    }

    struct ExecuteResolver {
        mode: ModeId,
        cmd: CommandId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for ExecuteResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Execute(
                self.cmd.clone(),
                reovim_driver_text_input::ResolveContext::default(),
            )
        }
    }

    let focus_cmd_id = CommandId::new(ModuleId::new("test"), "focus-win-cmd");

    let kernel = {
        use reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry};
        let services = Arc::new(ServiceRegistry::new());
        services.register(Arc::new(reovim_driver_text_buffer::TextBufferRegistry::new()));
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(reovim_driver_text_buffer::TestBufferManager::new()),
            Arc::new(OptionRegistry::new()),
            services,
        )
    };
    let state = crate::session::SessionState::with_kernel(kernel);
    let session =
        crate::session::Session::from_state(SessionId::new("presence-update-test"), state);
    let normal_mode = ModeId::new(ModuleId::new("default"), "normal");

    session.with_state_mut_sync(|state| {
        state.command_registry.register(Arc::new(FocusWindowCmd {
            id: focus_cmd_id.clone(),
        }));
        state.resolver_registry.register(ExecuteResolver {
            mode: normal_mode,
            cmd: focus_cmd_id,
        });
    });

    let buffer_id = session.with_state_mut_sync(|state| state.create_buffer("hello"));

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Join the client to the presence map so session.presence().update() executes
    // its closure body (covers L282-284: p.buffer_id = new_buffer_id).
    session
        .presence()
        .join(crate::session::ClientPresence::new(client_id, "tui", "focus-test"));

    // Give the client a window for the buffer
    session.clients().update_client_state(client_id, |state| {
        state.active_buffer = Some(buffer_id);
        let window = reovim_driver_text_session::Window::with_buffer(buffer_id);
        state.windows = reovim_driver_text_session::WindowLayout::single(window);
    });

    let registry = Arc::new(SessionRegistry::new());
    let session_arc = Arc::new(session);
    registry.insert(&session_arc);

    let service = InputServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("presence-update-test"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(
        SendKeysRequest {
            keys: "a".to_string(),
        },
        client_id,
    );
    let response = service.send_keys(request).await;
    assert!(response.is_ok());
}

// =========================================================================
// apply_mode_transition_for_client: completion loop Push/Set branches
// (L989-999: Push/Set in mode stack update; L1007-1013: nested Pop+result)
// =========================================================================

/// Helper: create a `CompletionPushResolver` whose `on_command_complete` returns
/// `ModeTransition::Push { mode: target_mode }`.
fn session_with_push_completion_resolver(
    name: &str,
    target_mode_name: &'static str,
) -> crate::session::Session {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput, TransitionContext,
        },
        reovim_driver_text_session::{ExtensionMap, api::SessionApiDyn},
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    struct PushCompletionResolver {
        mode: ModeId,
        push_target: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for PushCompletionResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }

        fn on_command_complete(
            &self,
            _session: &mut dyn SessionApiDyn,
            _shared_extensions: &mut ExtensionMap,
            _client_extensions: &mut ExtensionMap,
        ) -> Option<reovim_driver_text_input::ModeTransition> {
            Some(reovim_driver_text_input::ModeTransition::Push {
                mode: self.push_target.clone(),
                context: TransitionContext::new(),
            })
        }
    }

    // Also register a resolver for the pushed mode so it doesn't panic.
    struct TargetModeResolver {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for TargetModeResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }
        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    let session = crate::session::Session::new(SessionId::new(name));
    let normal_mode = ModeId::new(ModuleId::new("default"), "normal");
    let target_mode = ModeId::new(ModuleId::new("test"), target_mode_name);

    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(PushCompletionResolver {
            mode: normal_mode,
            push_target: target_mode.clone(),
        });
        state
            .resolver_registry
            .register(TargetModeResolver { mode: target_mode });
    });

    session
}

#[tokio::test]
async fn test_apply_mode_transition_completion_loop_push() {
    // Exercises L992-994 (Push branch in the deferred-completion while loop).
    // and L1013 (break — after a Push, the else branch is taken).
    //
    // We call `apply_mode_transition_for_client` with `Pop { result: None }`.
    // Inside that function, the while loop calls `on_command_complete` on the
    // current mode's resolver, which returns `Push { mode: pending }`.
    // L992-994 is executed (stack.push), then `complete_transition` is Push
    // (not Pop+Some), so L1014 (`else { break }`) is taken.

    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let session = session_with_push_completion_resolver("completion-push-test", "pending");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Start in normal mode; push an extra mode so the Pop has something to pop.
    let extra = ModeId::new(ModuleId::new("test"), "extra-push");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(extra.clone());
    });

    // We register a resolver for "extra-push" so the while-loop's next
    // on_command_complete call (after the push lands us in "normal") uses
    // PushCompletionResolver which triggers the Push branch.
    // Actually, the pop brings us back to normal mode, which has PushCompletionResolver.

    // Call apply_mode_transition_for_client with Pop { result: None }:
    // 1. Pop: removes extra-push, leaves normal.
    // 2. while loop: on_command_complete for normal → returns Push { pending }.
    // 3. L992-994 runs; break at L1014.
    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Pop { result: None },
    )
    .await;

    // Mode should now be "pending" (pushed by on_command_complete)
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "pending");
}

/// Helper: session with a resolver whose `on_command_complete` returns
/// `ModeTransition::Set { mode: target }`.
fn session_with_set_completion_resolver(
    name: &str,
    target_mode_name: &'static str,
) -> crate::session::Session {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput, TransitionContext,
        },
        reovim_driver_text_session::{ExtensionMap, api::SessionApiDyn},
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    struct SetCompletionResolver {
        mode: ModeId,
        set_target: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for SetCompletionResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }
        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
        fn on_command_complete(
            &self,
            _session: &mut dyn SessionApiDyn,
            _shared_extensions: &mut ExtensionMap,
            _client_extensions: &mut ExtensionMap,
        ) -> Option<reovim_driver_text_input::ModeTransition> {
            Some(reovim_driver_text_input::ModeTransition::Set {
                mode: self.set_target.clone(),
                context: TransitionContext::new(),
            })
        }
    }

    struct TargetModeResolver2 {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for TargetModeResolver2 {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }
        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    let session = crate::session::Session::new(SessionId::new(name));
    let normal_mode = ModeId::new(ModuleId::new("default"), "normal");
    let target_mode = ModeId::new(ModuleId::new("test"), target_mode_name);

    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(SetCompletionResolver {
            mode: normal_mode,
            set_target: target_mode.clone(),
        });
        state
            .resolver_registry
            .register(TargetModeResolver2 { mode: target_mode });
    });

    session
}

#[allow(clippy::items_after_statements)]
#[tokio::test]
async fn test_apply_mode_transition_completion_loop_set() {
    // Exercises L995-999 (Set branch in the deferred-completion while loop)
    // and L1013 (break — after a Set, the else branch is taken).
    //
    // Same structure as the Push test but using SetCompletionResolver.

    let session = session_with_set_completion_resolver("completion-set-test", "visual");
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let extra = ModeId::new(ModuleId::new("test"), "extra-set");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(extra);
    });

    // Call apply_mode_transition_for_client with Pop { result: None }:
    // 1. Pop: removes extra-set, leaves normal.
    // 2. while loop: on_command_complete for normal → returns Set { visual }.
    // 3. L995-999 runs; break at L1014.
    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Pop { result: None },
    )
    .await;

    // Mode should now be "visual" (set by on_command_complete)
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "visual");
}

/// Session with a resolver whose `on_command_complete` returns
/// `Pop { result: Some(ExecuteCommand { ... }) }` on the first call and
/// `None` on all subsequent calls. This exercises L1007-1012 without
/// creating an infinite loop.
#[allow(clippy::items_after_statements)]
fn session_with_pop_result_completion_resolver(name: &str) -> crate::session::Session {
    use {
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput,
        },
        reovim_driver_text_session::{ExtensionMap, SessionRuntime, api::SessionApiDyn},
        reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
    };

    // Extension that tracks whether on_command_complete has already fired.
    #[derive(Default)]
    struct OnceState {
        fired: bool,
    }
    impl reovim_driver_text_session::SessionExtension for OnceState {
        fn create() -> Self {
            Self { fired: false }
        }
    }

    // The nested command executed by the Pop result.
    let nested_cmd_id = CommandId::new(ModuleId::new("test"), "nested-pop-cmd");

    struct NestedCmd {
        id: CommandId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for NestedCmd {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            "nested"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &[]
        }
    }

    impl CommandHandler for NestedCmd {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &reovim_subsys_command_types::CommandContext,
        ) -> reovim_subsys_command_types::CommandResult {
            reovim_subsys_command_types::CommandResult::Success
        }
    }

    struct OncePopResultResolver {
        mode: ModeId,
        nested_cmd: CommandId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for OncePopResultResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }

        fn on_command_complete(
            &self,
            _session: &mut dyn SessionApiDyn,
            _shared_extensions: &mut ExtensionMap,
            client_extensions: &mut ExtensionMap,
        ) -> Option<reovim_driver_text_input::ModeTransition> {
            let state = client_extensions.get_or_insert::<OnceState>();
            if state.fired {
                return None;
            }
            state.fired = true;
            // Return Pop with a nested ExecuteCommand — exercises L1007-1012.
            Some(reovim_driver_text_input::ModeTransition::Pop {
                result: Some(reovim_driver_text_input::PopResult::ExecuteCommand {
                    command: self.nested_cmd.clone(),
                    args: std::collections::HashMap::new(),
                }),
            })
        }
    }

    let session = crate::session::Session::new(SessionId::new(name));
    let normal_mode = ModeId::new(ModuleId::new("default"), "normal");

    session.with_state_mut_sync(|state| {
        state.command_registry.register(Arc::new(NestedCmd {
            id: nested_cmd_id.clone(),
        }));
        state.resolver_registry.register(OncePopResultResolver {
            mode: normal_mode,
            nested_cmd: nested_cmd_id,
        });
    });

    session
}

#[allow(clippy::items_after_statements)]
#[tokio::test]
async fn test_apply_mode_transition_completion_loop_pop_with_nested_result() {
    // Exercises L1007-1012 (Pop { result: Some(nested) } path in the while loop)
    // and L988-990 (Pop branch update in the while loop).
    //
    // We call apply_mode_transition_for_client directly with Pop { result: None }.
    // The while loop runs with OncePopResultResolver which on first call returns
    // Pop { result: Some(ExecuteCommand) } → L1007-1012 is covered.
    // On the second iteration, the resolver returns None → loop exits.

    let session = session_with_pop_result_completion_resolver("completion-pop-result-test");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push a mode so the Pop in on_command_complete (inside the while loop)
    // has something to pop (depth > 1 → L988-990).
    use reovim_kernel::api::v1::{ModeId, ModuleId};
    let extra_mode = ModeId::new(ModuleId::new("test"), "extra-pop-result");
    session.clients().update_client_state(client_id, |state| {
        state.mode_stack.push(extra_mode.clone());
    });

    // Call apply_mode_transition_for_client with Pop { result: None }.
    // Inside the function:
    //   1. First Pop (the passed-in transition): pops extra-pop-result → back to normal.
    //   2. while loop: OncePopResultResolver returns Pop { result: Some(nested_cmd) }
    //      L988-990 (depth check/pop) + L1007-1012 (execute nested) are covered.
    //   3. Second while loop iteration: OncePopResultResolver returns None → exits.
    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Pop { result: None },
    )
    .await;

    // Should be in normal mode after everything
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

// =========================================================================
// apply_mode_transition_for_client: while-loop Pop branch with depth > 1
// (L989: mode_stack.pop() inside the Pop arm of the while-loop closure)
// =========================================================================

/// Helper: a session whose resolver for `pushed-pop-mode` returns
/// `Pop { result: None }` from `on_command_complete`.
fn session_with_pop_on_complete_resolver(name: &str) -> crate::session::Session {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput,
        },
        reovim_driver_text_session::{ExtensionMap, api::SessionApiDyn},
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    struct PopOnCompleteResolver {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for PopOnCompleteResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }

        fn on_command_complete(
            &self,
            _session: &mut dyn SessionApiDyn,
            _shared_extensions: &mut ExtensionMap,
            _client_extensions: &mut ExtensionMap,
        ) -> Option<reovim_driver_text_input::ModeTransition> {
            Some(reovim_driver_text_input::ModeTransition::Pop { result: None })
        }
    }

    let session = crate::session::Session::new(SessionId::new(name));
    let pushed_mode = ModeId::new(ModuleId::new("test"), "pushed-pop-mode");

    session.with_state_mut_sync(|state| {
        state
            .resolver_registry
            .register(PopOnCompleteResolver { mode: pushed_mode });
    });

    session
}

#[allow(clippy::items_after_statements)]
#[tokio::test]
async fn test_apply_mode_transition_completion_loop_pop_stack_depth() {
    // Exercises L989: `mode_stack.pop()` inside the Pop arm of the while-loop
    // closure, when `depth > 1`.
    //
    // Flow:
    //   1. Initial transition: `Push { mode: pushed-pop-mode }`.
    //      After push: stack depth = 2 (normal + pushed-pop-mode).
    //   2. While loop: `on_command_complete` for `pushed-pop-mode` returns
    //      `Pop { result: None }`.
    //   3. Closure (L987-990): depth is 2 > 1 → L989 pop() called → depth 1.
    //   4. `complete_transition` is `Pop { result: None }` → break (L1014).
    let session = session_with_pop_on_complete_resolver("completion-pop-depth-test");
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let pushed_mode = ModeId::new(ModuleId::new("test"), "pushed-pop-mode");

    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Push {
            mode: pushed_mode,
            context: TransitionContext::new(),
        },
    )
    .await;

    // After Push + while-loop Pop: stack is popped back to "normal".
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "normal");
}

// =========================================================================
// apply_mode_transition_for_client: while-loop Set branch with depth > 1
// (L997-998: pop() inside `while depth > 1 { pop() }` in the Set arm)
// =========================================================================

/// Helper: a session whose resolver for `pushed-set-mode` returns
/// `Set { mode: "set-target" }` from `on_command_complete`.
fn session_with_set_on_complete_resolver(name: &str) -> crate::session::Session {
    use {
        reovim_driver_text_input::{
            KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ResolveInput, TransitionContext,
        },
        reovim_driver_text_session::{ExtensionMap, api::SessionApiDyn},
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    struct SetOnCompleteResolver {
        mode: ModeId,
        set_target: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for SetOnCompleteResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }

        fn on_command_complete(
            &self,
            _session: &mut dyn SessionApiDyn,
            _shared_extensions: &mut ExtensionMap,
            _client_extensions: &mut ExtensionMap,
        ) -> Option<reovim_driver_text_input::ModeTransition> {
            Some(reovim_driver_text_input::ModeTransition::Set {
                mode: self.set_target.clone(),
                context: TransitionContext::new(),
            })
        }
    }

    struct SetTargetResolver {
        mode: ModeId,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeKeyResolver for SetTargetResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &DriverKeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    let session = crate::session::Session::new(SessionId::new(name));
    // Use distinct module IDs so the ModeIds have distinct (module, discriminant) keys
    // and do not collide in the resolver registry hash map.
    let pushed_mode = ModeId::new(ModuleId::new("test-push"), "pushed-set-mode");
    let target_mode = ModeId::new(ModuleId::new("test-target"), "set-target");

    session.with_state_mut_sync(|state| {
        state.resolver_registry.register(SetOnCompleteResolver {
            mode: pushed_mode,
            set_target: target_mode.clone(),
        });
        state
            .resolver_registry
            .register(SetTargetResolver { mode: target_mode });
    });

    session
}

#[allow(clippy::items_after_statements)]
#[tokio::test]
async fn test_apply_mode_transition_completion_loop_set_stack_depth() {
    // Exercises L997-998: pop() inside `while depth > 1 { pop() }` in the Set
    // arm of the while-loop closure.
    //
    // Flow:
    //   1. Initial transition: `Push { mode: pushed-set-mode }`.
    //      After push: stack depth = 2 (normal + pushed-set-mode).
    //   2. While loop: `on_command_complete` for `pushed-set-mode` returns
    //      `Set { mode: "set-target" }`.
    //   3. Closure (L995-999): `while depth > 1 { pop() }` runs with depth=2
    //      → L997 pop() called → depth=1 → loop exits → L999 set("set-target").
    //   4. `complete_transition` is `Set { .. }` → break (L1014).
    let session = session_with_set_on_complete_resolver("completion-set-depth-test");
    use reovim_kernel::api::v1::{ModeId, ModuleId};

    let client_id = ClientId::new(1);
    session.add_client(client_id);
    // Must match the module ID used in session_with_set_on_complete_resolver.
    let pushed_mode = ModeId::new(ModuleId::new("test-push"), "pushed-set-mode");

    InputServiceImpl::apply_mode_transition_for_client(
        &session,
        client_id,
        ModeTransition::Push {
            mode: pushed_mode,
            context: TransitionContext::new(),
        },
    )
    .await;

    // After Push + while-loop Set: final mode should be "set-target".
    let mode = session.client_current_mode(client_id).unwrap();
    assert_eq!(mode.name(), "set-target");
}
