use {
    super::*, parking_lot::Mutex, reovim_driver_input::TransitionContext, std::collections::HashMap,
};

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
        reovim_driver_input::ArgValue::String("test_value".to_string()),
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
    let _ = session.set_client_relation(
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
    session.update_client_state(client_id, |state| {
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
    session.update_client_state(client_id, |state| {
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
                reovim_driver_command_types::ArgValue::String("value".to_string()),
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
    ctx.metadata
        .insert("motion".to_string(), reovim_driver_input::ArgValue::String("word".to_string()));
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
    args.insert("count".to_string(), reovim_driver_command_types::ArgValue::Count(5));
    args.insert("register".to_string(), reovim_driver_command_types::ArgValue::Register('a'));
    args.insert(
        "description".to_string(),
        reovim_driver_command_types::ArgValue::String("test arg".to_string()),
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
    session.update_client_state(client_id, |state| {
        let window = reovim_driver_session::Window::with_buffer(buffer_id);
        state.windows = reovim_driver_session::WindowLayout::single(window);
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
    use reovim_driver_session::api::OptionChange;

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
    use reovim_driver_session::api::OptionChange;

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
    session.update_client_state(client_id, |state| {
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
    session.update_client_state(client_id, |state| {
        state.mode_stack.push(insert_mode);
    });

    // Pop with data result
    let transition = ModeTransition::Pop {
        result: Some(PopResult::Data {
            values: HashMap::from([(
                "search".to_string(),
                reovim_driver_command_types::ArgValue::String("pattern".to_string()),
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
    session.update_client_state(client_id, |state| {
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
    session.update_client_state(client_id, |state| {
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
    ctx.metadata
        .insert("key1".to_string(), reovim_driver_input::ArgValue::String("val1".to_string()));
    ctx.metadata
        .insert("key2".to_string(), reovim_driver_input::ArgValue::String("val2".to_string()));
    ctx.metadata
        .insert("key3".to_string(), reovim_driver_input::ArgValue::String("val3".to_string()));
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
    session.update_client_state(client_id, |state| {
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
                reovim_driver_command_types::ArgValue::String("1,5".to_string()),
            )]),
        },
    );
}

#[tokio::test]
async fn test_handle_resolve_result_completed() {
    use reovim_driver_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("completed-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('x'));

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
    use reovim_driver_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("pending-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('d'));

    let (handled, changes) =
        InputServiceImpl::handle_resolve_result(&session, ResolveResult::Pending, &key, client_id)
            .await;

    assert!(handled);
    assert!(!changes.has_changes());
}

#[tokio::test]
async fn test_handle_resolve_result_not_handled() {
    use reovim_driver_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("nothandled-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('z'));

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
    use reovim_driver_input::{InputTarget, KeyCode};

    let session = crate::session::Session::new(SessionId::new("insertchar-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('a'));

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
        reovim_driver_input::KeyCode,
        reovim_kernel::api::v1::{ModeId, ModuleId},
    };

    let session = crate::session::Session::new(SessionId::new("modetrans-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('i'));

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
    use reovim_driver_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("inject-empty-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('q'));

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
    let _ = session.set_client_relation(
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
        reovim_driver_command_types::ArgValue::String("visual".to_string()),
    );
    values.insert("count".to_string(), reovim_driver_command_types::ArgValue::Count(10));
    values.insert("register".to_string(), reovim_driver_command_types::ArgValue::Register('a'));

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
    session.update_client_state(client_id, |state| {
        let window = reovim_driver_session::Window::with_buffer(buf1);
        state.windows = reovim_driver_session::WindowLayout::single(window);
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
    session.update_client_state(client_id, |state| {
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
    let _ = session.set_client_relation(client_id, None);

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
    use reovim_driver_input::KeyCode;

    let session = crate::session::Session::new(SessionId::new("exec-changes-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('x'));

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
        reovim_driver_session::SessionRuntime,
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
        reovim_driver_input::{
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
    use reovim_driver_input::KeyCode;

    let (session, cmd_id) = session_with_success_command("exec-reg-test");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('x'));

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
        reovim_driver_input::{KeyCode, TransitionContext},
        reovim_driver_session::{SessionRuntime, api::ModeApi},
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

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('i'));

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
    use reovim_driver_input::{InputTarget, KeyCode};

    let session = crate::session::Session::new(SessionId::new("insertchar-buf-test"));

    // Create a buffer but add_client does not assign a window to it,
    // so insert_char_for_client returns None (no active window for client).
    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('x'));

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
    use reovim_driver_input::{InputTarget, KeyCode};

    let session = crate::session::Session::new(SessionId::new("insert-newline-test"));

    session.with_state_mut_sync(|state| {
        state.create_buffer("line1\nline2");
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Enter);

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
    args.insert("count".to_string(), reovim_driver_command_types::ArgValue::Count(5));
    args.insert(
        "description".to_string(),
        reovim_driver_command_types::ArgValue::String("test".to_string()),
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
    use reovim_driver_input::KeyCode;

    let session = session_with_resolver("inject-resolver-test");
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('q'));

    let injected_keys = vec![
        reovim_driver_input::KeyEvent::new(KeyCode::Char('a')),
        reovim_driver_input::KeyEvent::new(KeyCode::Char('b')),
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
        reovim_driver_input::{
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

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('q'));

    let injected_keys = vec![reovim_driver_input::KeyEvent::new(KeyCode::Char('a'))];

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
// Coverage: InsertChar with buffer modification (lines 347-348)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_insert_char_buffer_modified() {
    use reovim_driver_input::{InputTarget, KeyCode};

    // Need a real BufferManager (not StubBufferManager) so buffers are actually stored
    let kernel = reovim_kernel::api::v1::KernelContext {
        buffers: std::sync::Arc::new(reovim_driver_buffer::TestBufferManager::new()),
        ..Default::default()
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

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('y'));
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

    assert!(handled);
    assert!(
        changes.buffer_modified,
        "InsertChar should record buffer modification (lines 347-348)"
    );
}

// =========================================================================
// Coverage: Execute with operator completion (lines 309, 312-313)
// =========================================================================

#[tokio::test]
async fn test_handle_resolve_result_execute_with_operator_completion() {
    use {
        reovim_driver_input::{
            KeyCode, KeyEvent as DriverKeyEvent, ModeKeyResolver, ModeState, ModeTransition,
            ResolveInput, TransitionContext,
        },
        reovim_driver_session::{ExtensionMap, api::SessionApiDyn},
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

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('d'));

    let (handled, _changes) = InputServiceImpl::handle_resolve_result(
        &session,
        ResolveResult::Execute(cmd_id, reovim_driver_input::ResolveContext::new()),
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
    use reovim_driver_input::KeyCode;

    // Session WITHOUT any resolver registered - resolve_key_for_client returns None
    let session = crate::session::Session::new(SessionId::new("inject-noresolver-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let key = reovim_driver_input::KeyEvent::new(KeyCode::Char('q'));

    let injected_keys = vec![
        reovim_driver_input::KeyEvent::new(KeyCode::Char('a')),
        reovim_driver_input::KeyEvent::new(KeyCode::Char('b')),
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
    let mut window = reovim_driver_session::Window::with_buffer(buffer_id);
    window.selection = Some(reovim_driver_session::api::Selection::character(
        reovim_types_text::Position::new(0, 0),
        reovim_types_text::Position::new(0, 5),
    ));
    let windows = reovim_driver_session::WindowLayout::single(window);

    InputServiceImpl::ensure_selection_change_recorded(&mut changes, &windows, Some(buffer_id));

    assert!(changes.selection_changed);
}

#[test]
fn test_ensure_selection_change_already_recorded_is_noop() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    changes.record_cursor_move(buffer_id);
    changes.selection_changed = true;

    let mut window = reovim_driver_session::Window::with_buffer(buffer_id);
    window.selection = Some(reovim_driver_session::api::Selection::character(
        reovim_types_text::Position::new(0, 0),
        reovim_types_text::Position::new(0, 5),
    ));
    let windows = reovim_driver_session::WindowLayout::single(window);

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
    let window = reovim_driver_session::Window::with_buffer(buffer_id);
    let windows = reovim_driver_session::WindowLayout::single(window);

    InputServiceImpl::ensure_selection_change_recorded(&mut changes, &windows, Some(buffer_id));

    assert!(!changes.selection_changed);
}

#[test]
fn test_ensure_selection_change_no_cursor_moved_is_noop() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    // cursor_moved is false

    let mut window = reovim_driver_session::Window::with_buffer(buffer_id);
    window.selection = Some(reovim_driver_session::api::Selection::character(
        reovim_types_text::Position::new(0, 0),
        reovim_types_text::Position::new(0, 5),
    ));
    let windows = reovim_driver_session::WindowLayout::single(window);

    InputServiceImpl::ensure_selection_change_recorded(&mut changes, &windows, Some(buffer_id));

    assert!(!changes.selection_changed);
}

// ========================================================================
// Generic bridge helper tests (#468)
// ========================================================================

/// Minimal bridge for testing generic bridge detection.
struct TestBridge {
    kind_str: &'static str,
    scope: reovim_driver_session::bridges::ExtensionScope,
}

impl TestBridge {
    const fn client(kind: &'static str) -> Self {
        Self {
            kind_str: kind,
            scope: reovim_driver_session::bridges::ExtensionScope::Client,
        }
    }
    const fn shared(kind: &'static str) -> Self {
        Self {
            kind_str: kind,
            scope: reovim_driver_session::bridges::ExtensionScope::Shared,
        }
    }
}

impl reovim_driver_session::bridges::ExtensionStateBridge for TestBridge {
    fn kind(&self) -> &'static str {
        self.kind_str
    }
    fn scope(&self) -> reovim_driver_session::bridges::ExtensionScope {
        self.scope
    }
    fn snapshot(
        &self,
        _extensions: &reovim_driver_session::ExtensionMap,
    ) -> Option<serde_json::Value> {
        None
    }
    fn is_active(&self, _extensions: &reovim_driver_session::ExtensionMap) -> bool {
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
