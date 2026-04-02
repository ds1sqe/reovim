use std::collections::HashMap;

use {
    reovim_driver_command_types::ArgValue as CmdArgValue,
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
    reovim_types_text::Position,
};

use crate::{
    ArgValue, InputTarget, KeyCode, KeyEvent, KeySequence, ModeKeyResolver, ModeState,
    ModeTransition, OperatorArgs, PopResult, ResolveContext, ResolveInput, ResolveResult,
    TransitionContext,
};

fn test_module() -> ModuleId {
    ModuleId::new("test")
}

fn test_mode() -> ModeId {
    ModeId::new(test_module(), "normal")
}

fn test_command() -> CommandId {
    CommandId::new(test_module(), "delete")
}

// ========================================================================
// Trait object safety tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_key_resolver_is_object_safe() {
    // Verify the trait is object-safe by accepting trait objects
    fn _accepts_ref(_: &dyn ModeKeyResolver) {}
    fn _accepts_box(_: Box<dyn ModeKeyResolver>) {}
    fn _accepts_arc(_: std::sync::Arc<dyn ModeKeyResolver>) {}
}

// ========================================================================
// ResolveResult tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_execute() {
    let cmd = test_command();
    let ctx = ResolveContext::new().count(3);
    let result = ResolveResult::Execute(cmd.clone(), ctx);

    if let ResolveResult::Execute(c, context) = result {
        assert_eq!(c, cmd);
        assert_eq!(context.count, Some(3));
    } else {
        panic!("expected Execute variant");
    }
}

#[test]
fn test_resolve_result_pending() {
    let result = ResolveResult::Pending;
    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
fn test_resolve_result_not_handled() {
    let result = ResolveResult::NotHandled;
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_insert_char() {
    // Test the helper method (buffer target)
    let result = ResolveResult::insert_char('x');
    if let ResolveResult::InsertChar { char: c, target } = result {
        assert_eq!(c, 'x');
        assert_eq!(target, InputTarget::Buffer);
    } else {
        panic!("expected InsertChar variant");
    }
}

#[test]
fn test_input_target_default_is_buffer() {
    let target = InputTarget::default();
    assert_eq!(target, InputTarget::Buffer);
}

#[test]
fn test_input_target_extension_type_id() {
    // Use TestExtension as a stand-in (it needs SessionExtension + TextInputSink)
    // For now, just test that Extension variant works
    let type_id = std::any::TypeId::of::<String>();
    let target = InputTarget::Extension(type_id);
    assert!(matches!(target, InputTarget::Extension(_)));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_insert_char_helper_creates_buffer_target() {
    let result = ResolveResult::insert_char('a');
    if let ResolveResult::InsertChar { char: c, target } = result {
        assert_eq!(c, 'a');
        assert_eq!(target, InputTarget::Buffer);
    } else {
        panic!("expected InsertChar with Buffer target");
    }
}

/// Test stub implementing `SessionExtension + TextInputSink` for
/// `InputTarget::extension::<T>()` tests, replacing the module-cmdline
/// dev-dependency.
struct MockTextInputSink;

impl reovim_driver_session::SessionExtension for MockTextInputSink {
    fn create() -> Self {
        Self
    }
}

impl reovim_driver_session::TextInputSink for MockTextInputSink {
    fn insert_char(&mut self, _ch: char) {}
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_insert_char_to_creates_extension_target() {
    let result = ResolveResult::insert_char_to::<MockTextInputSink>('x');
    if let ResolveResult::InsertChar { char: c, target } = result {
        assert_eq!(c, 'x');
        let expected_type_id = std::any::TypeId::of::<MockTextInputSink>();
        assert_eq!(target, InputTarget::Extension(expected_type_id));
    } else {
        panic!("expected InsertChar with Extension target");
    }
}

#[test]
fn test_input_target_extension_creates_correct_type_id() {
    let target = InputTarget::extension::<MockTextInputSink>();
    let expected_type_id = std::any::TypeId::of::<MockTextInputSink>();
    assert_eq!(target, InputTarget::Extension(expected_type_id));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_mode_transition() {
    let mode = test_mode();
    let mode_clone = mode.clone();
    let result = ResolveResult::ModeTransition(ModeTransition::Set {
        mode,
        context: TransitionContext::new(),
    });

    if let ResolveResult::ModeTransition(ModeTransition::Set { mode: m, .. }) = result {
        assert_eq!(m, mode_clone);
    } else {
        panic!("expected ModeTransition::Set variant");
    }
}

// ========================================================================
// ResolveContext tests
// ========================================================================

#[test]
fn test_resolve_context_new() {
    let ctx = ResolveContext::new();
    assert!(ctx.count.is_none());
    assert!(ctx.register.is_none());
    assert!(ctx.keys.is_empty());
    assert!(ctx.metadata.is_empty());
}

#[test]
fn test_resolve_context_builder() {
    let ctx = ResolveContext::new()
        .count(5)
        .register('a')
        .with_metadata("linewise", ArgValue::Bool(true));

    assert_eq!(ctx.count, Some(5));
    assert_eq!(ctx.register, Some('a'));
    assert_eq!(ctx.metadata.get("linewise"), Some(&ArgValue::Bool(true)));
}

#[test]
fn test_resolve_context_effective_count() {
    let ctx_none = ResolveContext::new();
    assert_eq!(ctx_none.effective_count(), 1);

    let ctx_some = ResolveContext::with_count(7);
    assert_eq!(ctx_some.effective_count(), 7);
}

// ========================================================================
// ArgValue tests
// ========================================================================

#[test]
fn test_arg_value_from_bool() {
    let v: ArgValue = true.into();
    assert_eq!(v, ArgValue::Bool(true));
}

#[test]
fn test_arg_value_from_int() {
    let v: ArgValue = 42i64.into();
    assert_eq!(v, ArgValue::Int(42));
}

#[test]
fn test_arg_value_from_string() {
    let v: ArgValue = "hello".into();
    assert_eq!(v, ArgValue::String("hello".to_string()));
}

#[test]
fn test_arg_value_from_position() {
    let pos = Position::new(5, 10);
    let v: ArgValue = pos.into();
    assert_eq!(v, ArgValue::Position(Position::new(5, 10)));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_arg_value_range() {
    let v = ArgValue::Range {
        start: Position::new(0, 0),
        end: Position::new(5, 10),
        linewise: true,
    };

    if let ArgValue::Range {
        start,
        end,
        linewise,
    } = v
    {
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(5, 10));
        assert!(linewise);
    } else {
        panic!("expected Range variant");
    }
}

// ========================================================================
// ModeTransition tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_transition_push() {
    let mode = test_mode();
    let mode_clone = mode.clone();
    let op = test_command();
    let op_clone = op.clone();
    let trans = ModeTransition::Push {
        mode,
        context: TransitionContext::with_operator(op),
    };

    if let ModeTransition::Push { mode: m, context } = trans {
        assert_eq!(m, mode_clone);
        assert_eq!(context.pending_operator, Some(op_clone));
    } else {
        panic!("expected Push variant");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_transition_pop() {
    let mut args = HashMap::new();
    args.insert("linewise".to_string(), CmdArgValue::Bool(false));
    args.insert("count".to_string(), CmdArgValue::Count(2));
    args.insert("register".to_string(), CmdArgValue::Register('a'));

    let trans = ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand {
            command: test_command(),
            args,
        }),
    };

    if let ModeTransition::Pop { result: Some(r) } = trans {
        if let PopResult::ExecuteCommand { command, args } = r {
            assert_eq!(command, test_command());
            assert_eq!(args.get("linewise"), Some(&CmdArgValue::Bool(false)));
            assert_eq!(args.get("count"), Some(&CmdArgValue::Count(2)));
            assert_eq!(args.get("register"), Some(&CmdArgValue::Register('a')));
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected Pop with result");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_transition_set() {
    let mode = test_mode();
    let trans = ModeTransition::Set {
        mode: mode.clone(),
        context: TransitionContext::new().count(3),
    };

    if let ModeTransition::Set { mode: m, context } = trans {
        assert_eq!(m, mode);
        assert_eq!(context.count, Some(3));
    } else {
        panic!("expected Set variant");
    }
}

// TransitionContext and PopResult tests are in reovim-driver-session::transition (canonical source)

// ========================================================================
// ModeState tests
// ========================================================================

#[test]
fn test_mode_state_new() {
    let mode = test_mode();
    let state = ModeState::new(mode.clone());

    assert_eq!(state.current_mode(), &mode);
    assert!(!state.has_pending_keys());
    assert!(state.active_buffer.is_none());
    assert!(state.transition_context.is_none());
}

#[test]
fn test_mode_state_pending_keys() {
    let mode = test_mode();
    let mut state = ModeState::new(mode);

    assert!(!state.has_pending_keys());

    state.push_pending_key(KeyEvent::new(KeyCode::Char('g')));
    assert!(state.has_pending_keys());
    assert_eq!(state.pending_keys.len(), 1);

    state.clear_pending_keys();
    assert!(!state.has_pending_keys());
}

#[test]
fn test_mode_state_take_transition_context() {
    let mode = test_mode();
    let mut state = ModeState::new(mode);

    state.transition_context = Some(TransitionContext::new().count(5));
    assert!(state.transition_context.is_some());

    let ctx = state.take_transition_context();
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().count, Some(5));
    assert!(state.transition_context.is_none());
}

// ========================================================================
// OperatorArgs tests (#391)
// ========================================================================

#[test]
fn test_operator_args_new() {
    let args = OperatorArgs::new();
    assert!(args.count.is_none());
    assert!(args.register.is_none());
    assert!(args.is_empty());
}

#[test]
fn test_operator_args_with_count() {
    let args = OperatorArgs::with_count(5);
    assert_eq!(args.count, Some(5));
    assert!(args.register.is_none());
    assert!(!args.is_empty());
}

#[test]
fn test_operator_args_builder() {
    let args = OperatorArgs::new().count(3).register('a');
    assert_eq!(args.count, Some(3));
    assert_eq!(args.register, Some('a'));
    assert!(!args.is_empty());
}

#[test]
fn test_operator_args_effective_count() {
    let args_none = OperatorArgs::new();
    assert_eq!(args_none.effective_count(), 1);

    let args_some = OperatorArgs::with_count(7);
    assert_eq!(args_some.effective_count(), 7);
}

#[test]
fn test_operator_args_is_empty() {
    let empty = OperatorArgs::new();
    assert!(empty.is_empty());

    let with_count = OperatorArgs::new().count(1);
    assert!(!with_count.is_empty());

    let with_register = OperatorArgs::new().register('b');
    assert!(!with_register.is_empty());
}

#[test]
fn test_operator_args_equality() {
    let args1 = OperatorArgs::new().count(3).register('a');
    let args2 = OperatorArgs::new().count(3).register('a');
    let args3 = OperatorArgs::new().count(3).register('b');

    assert_eq!(args1, args2);
    assert_ne!(args1, args3);
}

#[test]
fn test_operator_args_default() {
    let args = OperatorArgs::default();
    assert!(args.is_empty());
    assert_eq!(args.effective_count(), 1);
}

// ========================================================================
// Additional ArgValue conversion tests
// ========================================================================

#[test]
fn test_arg_value_from_uint() {
    let v: ArgValue = 42u64.into();
    assert_eq!(v, ArgValue::Uint(42));
}

#[test]
fn test_arg_value_from_float() {
    let v: ArgValue = 2.71f64.into();
    assert_eq!(v, ArgValue::Float(2.71));
}

#[test]
fn test_arg_value_from_char() {
    let v: ArgValue = 'x'.into();
    assert_eq!(v, ArgValue::Char('x'));
}

#[test]
fn test_arg_value_from_owned_string() {
    let v: ArgValue = String::from("hello").into();
    assert_eq!(v, ArgValue::String("hello".to_string()));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_arg_value_debug() {
    let v = ArgValue::Bool(true);
    let debug = format!("{v:?}");
    assert!(debug.contains("Bool"));
    assert!(debug.contains("true"));
}

#[test]
fn test_arg_value_range_equality() {
    let v1 = ArgValue::Range {
        start: Position::new(0, 0),
        end: Position::new(5, 10),
        linewise: true,
    };
    let v2 = ArgValue::Range {
        start: Position::new(0, 0),
        end: Position::new(5, 10),
        linewise: true,
    };
    let v3 = ArgValue::Range {
        start: Position::new(0, 0),
        end: Position::new(5, 10),
        linewise: false,
    };
    assert_eq!(v1, v2);
    assert_ne!(v1, v3);
}

// ========================================================================
// ResolveContext additional tests
// ========================================================================

#[test]
fn test_resolve_context_keys() {
    let keys = KeySequence::from_keys(&[
        KeyEvent::new(KeyCode::Char('d')),
        KeyEvent::new(KeyCode::Char('w')),
    ]);
    let ctx = ResolveContext::new().keys(keys.clone());
    assert_eq!(ctx.keys, keys);
}

#[test]
fn test_resolve_context_register_builder() {
    let ctx = ResolveContext::new().register('z');
    assert_eq!(ctx.register, Some('z'));
}

#[test]
fn test_resolve_context_with_metadata_multiple() {
    let ctx = ResolveContext::new()
        .with_metadata("linewise", ArgValue::Bool(true))
        .with_metadata("count", ArgValue::Int(5));

    assert_eq!(ctx.metadata.get("linewise"), Some(&ArgValue::Bool(true)));
    assert_eq!(ctx.metadata.get("count"), Some(&ArgValue::Int(5)));
}

#[test]
fn test_resolve_context_default() {
    let ctx = ResolveContext::default();
    assert!(ctx.count.is_none());
    assert!(ctx.register.is_none());
    assert!(ctx.keys.is_empty());
    assert!(ctx.metadata.is_empty());
}

// ========================================================================
// ResolveResult variant tests
// ========================================================================

#[test]
fn test_resolve_result_completed() {
    let result = ResolveResult::Completed;
    assert!(matches!(result, ResolveResult::Completed));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_inject_keys() {
    let keys = vec![
        KeyEvent::new(KeyCode::Char('d')),
        KeyEvent::new(KeyCode::Char('w')),
    ];
    let result = ResolveResult::InjectKeys {
        keys,
        exit_macro_playback: true,
    };
    if let ResolveResult::InjectKeys {
        keys: k,
        exit_macro_playback,
    } = result
    {
        assert_eq!(k.len(), 2);
        assert!(exit_macro_playback);
    } else {
        panic!("expected InjectKeys variant");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_inject_keys_no_exit() {
    let result = ResolveResult::InjectKeys {
        keys: vec![],
        exit_macro_playback: false,
    };
    if let ResolveResult::InjectKeys {
        keys,
        exit_macro_playback,
    } = result
    {
        assert!(keys.is_empty());
        assert!(!exit_macro_playback);
    } else {
        panic!("expected InjectKeys variant");
    }
}

// ========================================================================
// ModeTransition additional tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_transition_pop_none_result() {
    let trans = ModeTransition::Pop { result: None };
    if let ModeTransition::Pop { result } = trans {
        assert!(result.is_none());
    } else {
        panic!("expected Pop variant");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_transition_debug() {
    let mode = test_mode();
    let trans = ModeTransition::Push {
        mode,
        context: TransitionContext::new(),
    };
    let debug = format!("{trans:?}");
    assert!(debug.contains("Push"));
}

// ========================================================================
// InputTarget additional tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_input_target_debug() {
    let target = InputTarget::Buffer;
    let debug = format!("{target:?}");
    assert!(debug.contains("Buffer"));
}

#[test]
fn test_input_target_clone() {
    let target = InputTarget::Buffer;
    let cloned = target;
    assert_eq!(target, cloned);
}

// ========================================================================
// ModeState additional tests
// ========================================================================

#[test]
fn test_mode_state_active_buffer() {
    let mode = test_mode();
    let mut state = ModeState::new(mode);
    assert!(state.active_buffer.is_none());

    state.active_buffer = Some(reovim_kernel::api::v1::BufferId::from_raw(42));
    assert_eq!(state.active_buffer, Some(reovim_kernel::api::v1::BufferId::from_raw(42)));
}

#[test]
fn test_mode_state_multiple_pending_keys() {
    let mode = test_mode();
    let mut state = ModeState::new(mode);

    state.push_pending_key(KeyEvent::new(KeyCode::Char('d')));
    state.push_pending_key(KeyEvent::new(KeyCode::Char('w')));
    assert_eq!(state.pending_keys.len(), 2);
    assert!(state.has_pending_keys());

    state.clear_pending_keys();
    assert!(!state.has_pending_keys());
    assert_eq!(state.pending_keys.len(), 0);
}

#[test]
fn test_mode_state_clone() {
    let mode = test_mode();
    let mut state = ModeState::new(mode);
    state.push_pending_key(KeyEvent::new(KeyCode::Char('g')));
    state.active_buffer = Some(reovim_kernel::api::v1::BufferId::from_raw(1));

    let cloned = state.clone();
    assert_eq!(cloned.pending_keys.len(), 1);
    assert_eq!(cloned.active_buffer, Some(reovim_kernel::api::v1::BufferId::from_raw(1)));
}

// ========================================================================
// ResolveInput tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_input_new() {
    struct NoOpKeymap;
    impl crate::KeymapQuery for NoOpKeymap {
        fn query(
            &self,
            _mode: &reovim_kernel::api::v1::ModeId,
            _keys: &KeySequence,
        ) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    let keys = KeySequence::new();
    let mode = test_mode();
    let keymap = NoOpKeymap;
    let input = ResolveInput::new(&keys, &mode, &keymap);
    assert_eq!(input.keys, &keys);
    assert_eq!(input.mode, &mode);
    assert!(input.registers.is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_input_with_registers() {
    struct NoOpKeymap;
    impl crate::KeymapQuery for NoOpKeymap {
        fn query(
            &self,
            _mode: &reovim_kernel::api::v1::ModeId,
            _keys: &KeySequence,
        ) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    let keys = KeySequence::new();
    let mode = test_mode();
    let keymap = NoOpKeymap;
    let registers = std::sync::Arc::new(reovim_kernel::api::v1::RwLock::new(
        reovim_kernel::api::v1::RegisterBank::new(),
    ));

    let input = ResolveInput::with_registers(&keys, &mode, &keymap, &registers);
    assert!(input.registers.is_some());
}

// ========================================================================
// OperatorArgs additional tests
// ========================================================================

#[test]
fn test_operator_args_clone() {
    let args = OperatorArgs::new().count(3).register('a');
    let cloned = args.clone();
    assert_eq!(args, cloned);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_operator_args_debug() {
    let args = OperatorArgs::new().count(5);
    let debug = format!("{args:?}");
    assert!(debug.contains("OperatorArgs"));
    assert!(debug.contains('5'));
}

// ========================================================================
// ModeKeyResolver trait default implementations
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_default_inherits_from_is_none() {
    struct TestResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for TestResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }
    }

    let mode = test_mode();
    let resolver = TestResolver { mode };
    assert!(resolver.inherits_from().is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_reset_is_noop_by_default() {
    struct TestResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for TestResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }
    }

    let mode = test_mode();
    let mut resolver = TestResolver { mode };
    // Should not panic
    resolver.reset();
}

#[test]
fn test_resolver_pending_keys_default_empty() {
    struct TestResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for TestResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }
    }

    let resolver = TestResolver { mode: test_mode() };
    let keys = resolver.pending_keys();
    assert!(keys.is_empty());
}

// ========================================================================
// ModeKeyResolver default method delegation tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_resolve_with_keymap_delegates_to_resolve() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct CountingResolver {
        mode: ModeId,
        resolve_called: AtomicBool,
    }

    #[allow(deprecated)]
    impl ModeKeyResolver for CountingResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve(&self, _key: &KeyEvent, _state: &mut ModeState) -> ResolveResult {
            self.resolve_called.store(true, Ordering::SeqCst);
            ResolveResult::NotHandled
        }
    }

    struct NoOpKeymap;
    impl crate::KeymapQuery for NoOpKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    let resolver = CountingResolver {
        mode: test_mode(),
        resolve_called: AtomicBool::new(false),
    };
    let key = KeyEvent::new(KeyCode::Char('x'));
    let mut state = ModeState::new(test_mode());
    let keys = KeySequence::new();
    let mode = test_mode();
    let keymap = NoOpKeymap;
    let input = ResolveInput::new(&keys, &mode, &keymap);

    let result = resolver.resolve_with_keymap(&key, &mut state, &input);
    assert!(resolver.resolve_called.load(Ordering::SeqCst));
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_resolve_with_extensions_delegates_to_keymap() {
    struct DelegatingResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for DelegatingResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &KeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Pending
        }
    }

    struct NoOpKeymap;
    impl crate::KeymapQuery for NoOpKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    let resolver = DelegatingResolver { mode: test_mode() };
    let key = KeyEvent::new(KeyCode::Char('x'));
    let mut state = ModeState::new(test_mode());
    let keys = KeySequence::new();
    let mode = test_mode();
    let keymap = NoOpKeymap;
    let input = ResolveInput::new(&keys, &mode, &keymap);
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_ext = reovim_driver_session::ExtensionMap::new();

    let result = resolver.resolve_with_extensions(
        &key,
        &mut state,
        &input,
        &mut extensions,
        &mut client_ext,
    );
    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_resolve_with_session_delegates_to_extensions() {
    struct SessionResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for SessionResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_extensions(
            &self,
            _key: &KeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
            _shared_extensions: &mut reovim_driver_session::ExtensionMap,
            _client_extensions: &mut reovim_driver_session::ExtensionMap,
        ) -> ResolveResult {
            ResolveResult::Completed
        }
    }

    struct NoOpKeymap;
    impl crate::KeymapQuery for NoOpKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    let resolver = SessionResolver { mode: test_mode() };
    let key = KeyEvent::new(KeyCode::Char('x'));
    let mut state = ModeState::new(test_mode());
    let keys = KeySequence::new();
    let mode = test_mode();
    let keymap = NoOpKeymap;
    let input = ResolveInput::new(&keys, &mode, &keymap);
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_ext = reovim_driver_session::ExtensionMap::new();

    // Use a test session runtime to get a dyn SessionApiDyn
    let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();

    let result = resolver.resolve_with_session(
        &key,
        &mut state,
        &input,
        &mut runtime,
        &mut extensions,
        &mut client_ext,
    );
    assert!(matches!(result, ResolveResult::Completed));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_on_command_complete_returns_none_by_default() {
    struct TestResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for TestResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }
    }

    let resolver = TestResolver { mode: test_mode() };

    let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_ext = reovim_driver_session::ExtensionMap::new();

    let result = resolver.on_command_complete(&mut runtime, &mut extensions, &mut client_ext);
    assert!(result.is_none());
}

// ========================================================================
// ResolveContext additional edge case tests
// ========================================================================

#[test]
fn test_resolve_context_with_count_creates_count() {
    let ctx = ResolveContext::with_count(42);
    assert_eq!(ctx.count, Some(42));
    assert!(ctx.register.is_none());
    assert!(ctx.keys.is_empty());
    assert!(ctx.metadata.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_context_debug() {
    let ctx = ResolveContext::new().count(3).register('a');
    let debug = format!("{ctx:?}");
    assert!(debug.contains("ResolveContext"));
    assert!(debug.contains('3'));
    assert!(debug.contains('a'));
}

#[test]
fn test_resolve_context_clone() {
    let ctx = ResolveContext::new()
        .count(5)
        .register('z')
        .with_metadata("key", ArgValue::Bool(true));
    #[allow(clippy::redundant_clone)]
    let cloned = ctx.clone();
    assert_eq!(cloned.count, Some(5));
    assert_eq!(cloned.register, Some('z'));
    assert_eq!(cloned.metadata.get("key"), Some(&ArgValue::Bool(true)));
}

// ========================================================================
// ModeState additional tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_state_debug() {
    let state = ModeState::new(test_mode());
    let debug = format!("{state:?}");
    assert!(debug.contains("ModeState"));
    assert!(debug.contains("pending_keys"));
}

// ========================================================================
// ModeTransition clone and debug
// ========================================================================

#[test]
fn test_mode_transition_clone() {
    let trans = ModeTransition::Push {
        mode: test_mode(),
        context: TransitionContext::new(),
    };
    #[allow(clippy::redundant_clone)]
    let _cloned = trans.clone();
    // Should not panic
}

#[test]
fn test_mode_transition_pop_with_cancelled() {
    let trans = ModeTransition::Pop {
        result: Some(PopResult::Cancelled),
    };
    assert!(matches!(
        trans,
        ModeTransition::Pop {
            result: Some(PopResult::Cancelled),
        }
    ));
}

// ========================================================================
// ResolveResult clone and debug
// ========================================================================

#[test]
fn test_resolve_result_clone() {
    let result = ResolveResult::Execute(test_command(), ResolveContext::new().count(3));
    #[allow(clippy::redundant_clone)]
    let _cloned = result.clone();
    // Should not panic
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_debug() {
    let result = ResolveResult::Completed;
    let debug = format!("{result:?}");
    assert!(debug.contains("Completed"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_insert_char_debug() {
    let result = ResolveResult::insert_char('z');
    let debug = format!("{result:?}");
    assert!(debug.contains("InsertChar"));
    assert!(debug.contains('z'));
}

// ========================================================================
// InputTarget equality and debug
// ========================================================================

#[test]
fn test_input_target_extension_equality() {
    let type_id_1 = std::any::TypeId::of::<String>();
    let type_id_2 = std::any::TypeId::of::<String>();
    let type_id_3 = std::any::TypeId::of::<u32>();

    let t1 = InputTarget::Extension(type_id_1);
    let t2 = InputTarget::Extension(type_id_2);
    let t3 = InputTarget::Extension(type_id_3);

    assert_eq!(t1, t2);
    assert_ne!(t1, t3);
}

#[test]
fn test_input_target_buffer_ne_extension() {
    let type_id = std::any::TypeId::of::<String>();
    assert_ne!(InputTarget::Buffer, InputTarget::Extension(type_id));
}

// ========================================================================
// ArgValue additional tests
// ========================================================================

#[test]
fn test_arg_value_clone() {
    let v = ArgValue::String("hello".to_string());
    let cloned = v.clone();
    assert_eq!(v, cloned);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_arg_value_range_debug() {
    let v = ArgValue::Range {
        start: Position::new(0, 0),
        end: Position::new(5, 10),
        linewise: true,
    };
    let debug = format!("{v:?}");
    assert!(debug.contains("Range"));
    assert!(debug.contains("linewise"));
}

// ========================================================================
// OperatorArgs additional edge case
// ========================================================================

#[test]
fn test_operator_args_register_only() {
    let args = OperatorArgs::new().register('x');
    assert!(args.count.is_none());
    assert_eq!(args.register, Some('x'));
    assert!(!args.is_empty());
    assert_eq!(args.effective_count(), 1);
}

// ========================================================================
// Additional ModeKeyResolver default delegation chain tests
// ========================================================================

/// Resolver that only implements `resolve_with_keymap` to test that
/// `resolve_with_extensions` properly delegates to it.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_default_resolve_with_extensions_delegates_through_keymap_to_resolve() {
    // Test the full chain: resolve_with_extensions -> resolve_with_keymap -> resolve
    use std::sync::atomic::{AtomicU8, Ordering};

    struct ChainResolver {
        mode: ModeId,
        call_count: AtomicU8,
    }

    #[allow(deprecated)]
    impl ModeKeyResolver for ChainResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve(&self, _key: &KeyEvent, _state: &mut ModeState) -> ResolveResult {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            ResolveResult::insert_char('z')
        }
    }

    struct NoOpKeymap;
    impl crate::KeymapQuery for NoOpKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    let resolver = ChainResolver {
        mode: test_mode(),
        call_count: AtomicU8::new(0),
    };
    let key = KeyEvent::new(KeyCode::Char('a'));
    let mut state = ModeState::new(test_mode());
    let keys = KeySequence::new();
    let mode = test_mode();
    let keymap = NoOpKeymap;
    let input = ResolveInput::new(&keys, &mode, &keymap);
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_ext = reovim_driver_session::ExtensionMap::new();

    // Call resolve_with_extensions -> delegates to resolve_with_keymap -> delegates to resolve
    let result = resolver.resolve_with_extensions(
        &key,
        &mut state,
        &input,
        &mut extensions,
        &mut client_ext,
    );
    assert!(matches!(result, ResolveResult::InsertChar { char: 'z', .. }));
    assert_eq!(resolver.call_count.load(Ordering::SeqCst), 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolver_default_resolve_with_session_full_chain() {
    // Test that resolve_with_session -> resolve_with_extensions -> resolve_with_keymap
    // all default implementations chain correctly.

    struct FullChainResolver {
        mode: ModeId,
    }

    impl ModeKeyResolver for FullChainResolver {
        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn resolve_with_keymap(
            &self,
            _key: &KeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Execute(test_command(), ResolveContext::new().count(42))
        }
    }

    struct NoOpKeymap;
    impl crate::KeymapQuery for NoOpKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    let resolver = FullChainResolver { mode: test_mode() };
    let key = KeyEvent::new(KeyCode::Char('x'));
    let mut state = ModeState::new(test_mode());
    let keys = KeySequence::new();
    let mode = test_mode();
    let keymap = NoOpKeymap;
    let input = ResolveInput::new(&keys, &mode, &keymap);
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_ext = reovim_driver_session::ExtensionMap::new();
    let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();

    let result = resolver.resolve_with_session(
        &key,
        &mut state,
        &input,
        &mut runtime,
        &mut extensions,
        &mut client_ext,
    );
    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd, test_command());
        assert_eq!(ctx.count, Some(42));
    } else {
        panic!("expected Execute variant");
    }
}

// ========================================================================
// ModeTransition additional variant coverage
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_transition_push_with_count() {
    let trans = ModeTransition::Push {
        mode: test_mode(),
        context: TransitionContext::new().count(10),
    };
    if let ModeTransition::Push { context, .. } = trans {
        assert_eq!(context.count, Some(10));
    } else {
        panic!("expected Push");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mode_transition_set_with_operator() {
    let op = test_command();
    let trans = ModeTransition::Set {
        mode: test_mode(),
        context: TransitionContext::with_operator(op.clone()),
    };
    if let ModeTransition::Set { context, .. } = trans {
        assert_eq!(context.pending_operator, Some(op));
    } else {
        panic!("expected Set");
    }
}

// ========================================================================
// ResolveResult mode transition variant coverage
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_result_mode_transition_push() {
    let result = ResolveResult::ModeTransition(ModeTransition::Push {
        mode: test_mode(),
        context: TransitionContext::new(),
    });
    if let ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) = result {
        assert_eq!(mode, test_mode());
    } else {
        panic!("expected ModeTransition::Push");
    }
}

#[test]
fn test_resolve_result_mode_transition_pop() {
    let result = ResolveResult::ModeTransition(ModeTransition::Pop { result: None });
    assert!(matches!(
        result,
        ResolveResult::ModeTransition(ModeTransition::Pop { result: None })
    ));
}

// ========================================================================
// ArgValue additional From conversions
// ========================================================================

#[test]
fn test_arg_value_from_bool_false() {
    let v: ArgValue = false.into();
    assert_eq!(v, ArgValue::Bool(false));
}

#[test]
fn test_arg_value_from_negative_int() {
    let v: ArgValue = (-42i64).into();
    assert_eq!(v, ArgValue::Int(-42));
}

#[test]
fn test_arg_value_from_zero_uint() {
    let v: ArgValue = 0u64.into();
    assert_eq!(v, ArgValue::Uint(0));
}

#[test]
fn test_arg_value_from_float_negative() {
    let v: ArgValue = (-1.5f64).into();
    assert_eq!(v, ArgValue::Float(-1.5));
}

#[test]
fn test_arg_value_position_clone() {
    let v = ArgValue::Position(Position::new(3, 7));
    let cloned = v.clone();
    assert_eq!(v, cloned);
}

#[test]
fn test_arg_value_range_clone() {
    let v = ArgValue::Range {
        start: Position::new(0, 0),
        end: Position::new(5, 5),
        linewise: false,
    };
    let cloned = v.clone();
    assert_eq!(v, cloned);
}

// ========================================================================
// ResolveContext metadata edge cases
// ========================================================================

#[test]
fn test_resolve_context_metadata_overwrite() {
    let ctx = ResolveContext::new()
        .with_metadata("key", ArgValue::Int(1))
        .with_metadata("key", ArgValue::Int(2));

    // Last write wins
    assert_eq!(ctx.metadata.get("key"), Some(&ArgValue::Int(2)));
}

#[test]
fn test_resolve_context_effective_count_zero() {
    let ctx = ResolveContext::new().count(0);
    assert_eq!(ctx.effective_count(), 0);
}

// ========================================================================
// InputTarget additional coverage
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_input_target_extension_debug() {
    let type_id = std::any::TypeId::of::<String>();
    let target = InputTarget::Extension(type_id);
    let debug = format!("{target:?}");
    assert!(debug.contains("Extension"));
}

#[test]
fn test_input_target_copy_semantics() {
    let t1 = InputTarget::Buffer;
    let t2 = t1;
    let t3 = t1;
    assert_eq!(t2, t3);
}

// ========================================================================
// ModeState edge cases
// ========================================================================

#[test]
fn test_mode_state_take_transition_context_when_none() {
    let mut state = ModeState::new(test_mode());
    assert!(state.transition_context.is_none());
    let taken = state.take_transition_context();
    assert!(taken.is_none());
}

#[test]
fn test_mode_state_push_and_clear_multiple_times() {
    let mut state = ModeState::new(test_mode());

    state.push_pending_key(KeyEvent::new(KeyCode::Char('a')));
    state.push_pending_key(KeyEvent::new(KeyCode::Char('b')));
    assert_eq!(state.pending_keys.len(), 2);
    state.clear_pending_keys();
    assert!(!state.has_pending_keys());

    // Push again after clear
    state.push_pending_key(KeyEvent::new(KeyCode::Char('c')));
    assert_eq!(state.pending_keys.len(), 1);
}

// ========================================================================
// OperatorArgs edge cases
// ========================================================================

#[test]
fn test_operator_args_count_zero() {
    let args = OperatorArgs::with_count(0);
    assert_eq!(args.effective_count(), 0);
    assert!(!args.is_empty());
}

#[test]
fn test_operator_args_large_count() {
    let args = OperatorArgs::with_count(usize::MAX);
    assert_eq!(args.effective_count(), usize::MAX);
}
