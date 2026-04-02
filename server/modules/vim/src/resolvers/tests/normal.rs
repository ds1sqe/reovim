use {
    reovim_driver_input::{
        ArgValue, ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, KeymapQuery,
        ModeKeyResolver, ModeState, ModeTransition, Modifiers, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
};

use reovim_module_editor as editor;

use {
    super::super::normal::*,
    crate::{
        modes::VimMode,
        session_state::{PendingCharOp, VimSessionState},
    },
};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn key_with_mod(c: char, modifiers: Modifiers) -> KeyEvent {
    KeyEvent::with_modifiers(KeyCode::Char(c), modifiers)
}

fn test_state() -> ModeState {
    ModeState::new(VimMode::NORMAL_ID)
}

/// Mock keymap that always returns `NotFound` (no bindings).
struct NotFoundKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for NotFoundKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::NotFound
    }
}

fn resolve_input_notfound() -> ResolveInput<'static> {
    static KEYMAP: NotFoundKeymap = NotFoundKeymap;
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::NORMAL_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, &KEYMAP)
}

#[test]
fn test_new_resolver() {
    let resolver = VimNormalResolver::new();
    assert_eq!(resolver.mode_id(), &VimMode::NORMAL_ID);
    assert!(resolver.pending_count().is_none());
    assert!(resolver.pending_register().is_none());
}

#[test]
fn test_is_count_digit_first() {
    let resolver = VimNormalResolver::new();

    // 1-9 are count digits when no count yet
    for c in '1'..='9' {
        assert!(resolver.is_count_digit(&key(c)), "'{c}' should be count digit");
    }

    // 0 is NOT a count digit when no count (it's a motion)
    assert!(!resolver.is_count_digit(&key('0')));
}

#[test]
fn test_is_count_digit_subsequent() {
    let resolver = VimNormalResolver::new();

    // Accumulate a count first
    resolver.accumulate_count(&key('3'));
    assert_eq!(resolver.pending_count(), Some(3));

    // Now 0 IS a count digit
    assert!(resolver.is_count_digit(&key('0')));
}

#[test]
fn test_count_digit_requires_no_modifiers() {
    let resolver = VimNormalResolver::new();

    // Ctrl+3 is NOT a count digit
    assert!(!resolver.is_count_digit(&key_with_mod('3', Modifiers::CTRL)));

    // Alt+5 is NOT a count digit
    assert!(!resolver.is_count_digit(&key_with_mod('5', Modifiers::ALT)));
}

#[test]
fn test_accumulate_count() {
    let resolver = VimNormalResolver::new();

    resolver.accumulate_count(&key('2'));
    assert_eq!(resolver.pending_count(), Some(2));

    resolver.accumulate_count(&key('5'));
    assert_eq!(resolver.pending_count(), Some(25));

    resolver.accumulate_count(&key('0'));
    assert_eq!(resolver.pending_count(), Some(250));
}

#[test]
fn test_register_prefix() {
    assert!(VimNormalResolver::is_register_prefix(&key('"')));
    assert!(!VimNormalResolver::is_register_prefix(&key('a')));
    assert!(!VimNormalResolver::is_register_prefix(&key_with_mod('"', Modifiers::SHIFT)));
}

#[test]
fn test_waiting_for_register() {
    let resolver = VimNormalResolver::new();

    assert!(!resolver.is_waiting_for_register());

    // Set sentinel
    *resolver.pending_register.write().unwrap() = Some('"');
    assert!(resolver.is_waiting_for_register());

    // Set actual register
    *resolver.pending_register.write().unwrap() = Some('a');
    assert!(!resolver.is_waiting_for_register());
}

#[test]
fn test_handle_register_char_valid() {
    let resolver = VimNormalResolver::new();
    *resolver.pending_register.write().unwrap() = Some('"');

    let result = resolver.handle_register_char(&key('a'));
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.pending_register(), Some('a'));
}

#[test]
fn test_handle_register_char_invalid() {
    let resolver = VimNormalResolver::new();
    *resolver.pending_register.write().unwrap() = Some('"');

    // Space is not a valid register
    let result = resolver.handle_register_char(&key(' '));
    assert!(matches!(result, ResolveResult::NotHandled));
    assert!(resolver.pending_register().is_none());
}

#[test]
fn test_take_count() {
    let resolver = VimNormalResolver::new();

    resolver.accumulate_count(&key('5'));
    assert_eq!(resolver.take_count(), Some(5));
    assert!(resolver.pending_count().is_none());
}

#[test]
fn test_take_register() {
    let resolver = VimNormalResolver::new();

    *resolver.pending_register.write().unwrap() = Some('a');
    assert_eq!(resolver.take_register(), Some('a'));
    assert!(resolver.pending_register().is_none());
}

#[test]
fn test_take_register_ignores_sentinel() {
    let resolver = VimNormalResolver::new();

    *resolver.pending_register.write().unwrap() = Some('"');
    assert!(resolver.take_register().is_none());
}

#[test]
fn test_reset() {
    let mut resolver = VimNormalResolver::new();

    resolver.accumulate_count(&key('3'));
    *resolver.pending_register.write().unwrap() = Some('a');
    resolver.push_pending_key(key('d'));

    resolver.reset();

    assert!(resolver.pending_count().is_none());
    assert!(resolver.pending_register().is_none());
    assert!(resolver.get_pending_keys().is_empty());
}

#[test]
fn test_resolve_escape_resets() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let input = resolve_input_notfound();

    resolver.accumulate_count(&key('3'));
    *resolver.pending_register.write().unwrap() = Some('a');

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

    assert!(matches!(result, ResolveResult::NotHandled));
    assert!(resolver.pending_count().is_none());
    assert!(resolver.pending_register().is_none());
}

#[test]
fn test_resolve_count_digit() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let input = resolve_input_notfound();

    let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.pending_count(), Some(3));

    let result = resolver.resolve_with_keymap(&key('5'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.pending_count(), Some(35));
}

#[test]
fn test_resolve_register_prefix() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let input = resolve_input_notfound();

    let result = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert!(resolver.is_waiting_for_register());

    let result = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.pending_register(), Some('a'));
}

#[test]
fn test_mode_id() {
    let resolver = VimNormalResolver::new();
    assert_eq!(resolver.mode_id().name(), "normal");
}

#[test]
fn test_inherits_from() {
    let resolver = VimNormalResolver::new();
    assert!(resolver.inherits_from().is_none());
}

// ========================================================================
// Keymap-aware resolution tests (Epic #353 - Mechanism/Policy separation)
// ========================================================================
//
// These tests verify that resolve_with_keymap correctly applies Vim policy
// based on the KeyLookupState returned by the keymap.

/// Test module ID for creating command IDs.
const TEST_MODULE: ModuleId = ModuleId::new("test");

/// Mock keymap that returns a configurable `KeyLookupState`.
struct MockKeymap {
    response: KeyLookupState,
}

impl MockKeymap {
    fn exact_only(cmd: &'static str) -> Self {
        Self {
            response: KeyLookupState::ExactOnly(CommandId::new(TEST_MODULE, cmd)),
        }
    }

    fn exact_with_longer(cmd: &'static str) -> Self {
        Self {
            response: KeyLookupState::ExactWithLonger {
                exact: CommandId::new(TEST_MODULE, cmd),
            },
        }
    }

    fn prefix_only() -> Self {
        Self {
            response: KeyLookupState::PrefixOnly,
        }
    }

    fn not_found() -> Self {
        Self {
            response: KeyLookupState::NotFound,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for MockKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        self.response.clone()
    }
}

fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
    // Keys are managed by resolver, so we pass empty sequence here
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::NORMAL_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

// ------------------------------------------------------------------------
// Vim Policy Tests: KeyLookupState -> ResolveResult mapping
// ------------------------------------------------------------------------

#[test]
fn test_exact_with_longer_returns_pending() {
    // When keymap reports ExactWithLonger (d exists and dd exists),
    // Vim policy says wait for more keys.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_with_longer("delete");
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('d'), &mut state, &input);

    assert!(matches!(result, ResolveResult::Pending));
    // pending_keys should contain the key
    assert!(!resolver.get_pending_keys().is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_exact_only_returns_execute() {
    // When keymap reports ExactOnly (x exists, no xx),
    // Vim policy says execute immediately.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("delete-char");
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('x'), &mut state, &input);

    match result {
        ResolveResult::Execute(cmd, _ctx) => {
            assert_eq!(cmd.name(), "delete-char");
        }
        _ => panic!("Expected Execute, got {result:?}"),
    }
    // pending_keys should be cleared after execute
    assert!(resolver.get_pending_keys().is_empty());
}

#[test]
fn test_prefix_only_returns_pending() {
    // When keymap reports PrefixOnly (g is prefix for gg, but no binding for just g),
    // Vim policy says wait for more keys.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::prefix_only();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input);

    assert!(matches!(result, ResolveResult::Pending));
    // pending_keys should contain the key
    assert!(!resolver.get_pending_keys().is_empty());
}

#[test]
fn test_not_found_clears_and_returns_not_handled() {
    // When keymap reports NotFound (no binding for z),
    // Vim policy clears pending keys and returns NotHandled.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

    assert!(matches!(result, ResolveResult::NotHandled));
    // pending_keys should be cleared
    assert!(resolver.get_pending_keys().is_empty());
}

// ------------------------------------------------------------------------
// Context building tests: count and register flow to Execute
// ------------------------------------------------------------------------

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_count_flows_to_context() {
    // Count accumulated before command should appear in Execute context.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);

    // Accumulate count first
    let _ = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    assert_eq!(resolver.pending_count(), Some(3));

    // Now press 'j' which executes
    let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);

    match result {
        ResolveResult::Execute(_cmd, ctx) => {
            assert_eq!(ctx.count, Some(3));
        }
        _ => panic!("Expected Execute, got {result:?}"),
    }
    // Count should be cleared after execute
    assert!(resolver.pending_count().is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_register_flows_to_context() {
    // Register selected before command should appear in Execute context.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("paste");
    let input = resolve_input(&keymap);

    // Select register first
    let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
    assert!(resolver.is_waiting_for_register());
    let _ = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
    assert_eq!(resolver.pending_register(), Some('a'));

    // Now press 'p' which executes
    let result = resolver.resolve_with_keymap(&key('p'), &mut state, &input);

    match result {
        ResolveResult::Execute(_cmd, ctx) => {
            assert_eq!(ctx.register, Some('a'));
        }
        _ => panic!("Expected Execute, got {result:?}"),
    }
    // Register should be cleared after execute
    assert!(resolver.pending_register().is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_count_and_register_together() {
    // Both count and register should flow to context.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("delete-word");
    let input = resolve_input(&keymap);

    // "a3 - select register 'a', then count 3
    let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
    let _ = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
    let _ = resolver.resolve_with_keymap(&key('3'), &mut state, &input);

    assert_eq!(resolver.pending_register(), Some('a'));
    assert_eq!(resolver.pending_count(), Some(3));

    // Press 'w' which executes
    let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

    match result {
        ResolveResult::Execute(_cmd, ctx) => {
            assert_eq!(ctx.count, Some(3));
            assert_eq!(ctx.register, Some('a'));
        }
        _ => panic!("Expected Execute, got {result:?}"),
    }
}

// ------------------------------------------------------------------------
// State management tests
// ------------------------------------------------------------------------

#[test]
fn test_escape_clears_all_state() {
    // Escape should clear count, register, and pending keys.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    // Accumulate some state
    let _ = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
    let _ = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
    resolver.push_pending_key(key('d'));

    assert!(resolver.pending_count().is_some());
    assert!(resolver.pending_register().is_some());
    assert!(!resolver.get_pending_keys().is_empty());

    // Press escape
    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

    assert!(matches!(result, ResolveResult::NotHandled));
    assert!(resolver.pending_count().is_none());
    assert!(resolver.pending_register().is_none());
    assert!(resolver.get_pending_keys().is_empty());
}

#[test]
fn test_pending_keys_cleared_after_execute() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("delete-char");
    let input = resolve_input(&keymap);

    let _ = resolver.resolve_with_keymap(&key('x'), &mut state, &input);

    assert!(resolver.get_pending_keys().is_empty());
}

#[test]
fn test_pending_keys_cleared_after_not_found() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let _ = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

    assert!(resolver.get_pending_keys().is_empty());
}

#[test]
fn test_register_sentinel_not_in_context() {
    // The sentinel value '"' should not leak into the execute context.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("some-cmd");
    let input = resolve_input(&keymap);

    // Start register prefix but don't complete it
    let _ = resolver.resolve_with_keymap(&key('"'), &mut state, &input);
    assert!(resolver.is_waiting_for_register());

    // Directly test take_register filters sentinel
    let reg = resolver.take_register();
    assert!(reg.is_none(), "Sentinel should not be returned");
}

// ========================================================================
// Operator interception tests (Epic #415 - Operator-pending mode)
// ========================================================================
//
// These tests verify that operator commands (d, y, c) are intercepted and
// return ModeTransition::Push to operator-pending mode.

/// Module ID for editor commands.
const EDITOR_MODULE: ModuleId = ModuleId::new("editor");

/// Mock keymap that returns operator commands.
impl MockKeymap {
    fn enter_delete_operator() -> Self {
        Self {
            response: KeyLookupState::ExactOnly(CommandId::new(
                EDITOR_MODULE,
                "enter-delete-operator",
            )),
        }
    }

    fn enter_yank_operator() -> Self {
        Self {
            response: KeyLookupState::ExactOnly(CommandId::new(
                EDITOR_MODULE,
                "enter-yank-operator",
            )),
        }
    }

    fn enter_change_operator() -> Self {
        Self {
            response: KeyLookupState::ExactOnly(CommandId::new(
                EDITOR_MODULE,
                "enter-change-operator",
            )),
        }
    }
}

fn resolve_with_ext(
    resolver: &VimNormalResolver,
    key: &KeyEvent,
    state: &mut ModeState,
    input: &ResolveInput<'_>,
    extensions: &mut ExtensionMap,
) -> ResolveResult {
    let mut shared_ext = ExtensionMap::new();
    resolver.resolve_with_extensions(key, state, input, &mut shared_ext, extensions)
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_d_returns_mode_transition_push_to_delete_mode() {
    // Epic #415: Pressing 'd' should push to dedicated DELETE mode.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::enter_delete_operator();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

    match result {
        ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
            assert_eq!(mode, VimMode::DELETE_ID, "Should push to DELETE mode");
        }
        _ => panic!("Expected ModeTransition::Push, got {result:?}"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_y_returns_mode_transition_push_to_yank_mode() {
    // Epic #415: Pressing 'y' should push to dedicated YANK mode.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::enter_yank_operator();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('y'), &mut state, &input, &mut extensions);

    match result {
        ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
            assert_eq!(mode, VimMode::YANK_ID, "Should push to YANK mode");
        }
        _ => panic!("Expected ModeTransition::Push, got {result:?}"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_c_returns_mode_transition_push_to_change_mode() {
    // Epic #415: Pressing 'c' should push to dedicated CHANGE mode.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::enter_change_operator();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('c'), &mut state, &input, &mut extensions);

    match result {
        ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
            assert_eq!(mode, VimMode::CHANGE_ID, "Should push to CHANGE mode");
        }
        _ => panic!("Expected ModeTransition::Push, got {result:?}"),
    }
}

#[test]
fn test_count_preserved_for_operator_resolver() {
    // Epic #415: Normal mode does NOT consume pending_count.
    // The dedicated operator resolver reads it on first key.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::enter_delete_operator();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Initialize VimSessionState with a count
    {
        let vim = extensions.get_or_insert::<VimSessionState>();
        vim.pending_count = Some(3);
    }

    let _ = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

    // Count should still be in VimSessionState (not consumed by normal resolver)
    let vim = extensions
        .get::<VimSessionState>()
        .expect("VimSessionState should exist");
    assert_eq!(
        vim.pending_count,
        Some(3),
        "pending_count should be preserved for operator resolver"
    );
}

#[test]
fn test_register_preserved_for_operator_resolver() {
    // Epic #415: Normal mode does NOT consume pending_register.
    // The dedicated operator resolver reads it on first key.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::enter_delete_operator();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Initialize VimSessionState with a register
    {
        let vim = extensions.get_or_insert::<VimSessionState>();
        vim.pending_register = Some('a');
    }

    let _ = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

    // Register should still be in VimSessionState (not consumed by normal resolver)
    let vim = extensions
        .get::<VimSessionState>()
        .expect("VimSessionState should exist");
    assert_eq!(
        vim.pending_register,
        Some('a'),
        "pending_register should be preserved for operator resolver"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_count_and_register_preserved_together() {
    // Epic #415: Both count and register should remain for the dedicated resolver.
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::enter_delete_operator();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Initialize VimSessionState with both count and register
    {
        let vim = extensions.get_or_insert::<VimSessionState>();
        vim.pending_count = Some(3);
        vim.pending_register = Some('a');
    }

    let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);

    // Verify we push to DELETE mode
    if let ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) = result {
        assert_eq!(mode, VimMode::DELETE_ID, "Should push to DELETE mode");
    } else {
        panic!("Expected ModeTransition::Push");
    }

    // Both should still be in VimSessionState
    let vim = extensions
        .get::<VimSessionState>()
        .expect("VimSessionState should exist");
    assert_eq!(vim.pending_count, Some(3), "pending_count should be preserved");
    assert_eq!(vim.pending_register, Some('a'), "pending_register should be preserved");
}

// ========================================================================
// Extension-based resolution tests (resolve_with_extensions)
// ========================================================================

#[test]
fn test_ext_escape_clears_vim_state() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Set up VimSessionState with pending values
    {
        let vim = extensions.get_or_insert::<VimSessionState>();
        vim.pending_count = Some(5);
        vim.pending_register = Some('b');
    }

    let result = resolve_with_ext(
        &resolver,
        &KeyEvent::new(KeyCode::Escape),
        &mut state,
        &input,
        &mut extensions,
    );

    assert!(matches!(result, ResolveResult::NotHandled));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert!(vim.pending_count.is_none());
    assert!(vim.pending_register.is_none());
}

#[test]
fn test_ext_count_digit_accumulates() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('3'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_count, Some(3));

    let result = resolve_with_ext(&resolver, &key('5'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_count, Some(35));
}

#[test]
fn test_ext_register_prefix() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Press " to start register prefix
    let result = resolve_with_ext(&resolver, &key('"'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_register, Some('"'));

    // Press 'a' to select register
    let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_register, Some('a'));
}

#[test]
fn test_ext_register_prefix_invalid_char() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Set sentinel
    {
        let vim = extensions.get_or_insert::<VimSessionState>();
        vim.pending_register = Some('"');
    }

    // Space is not a valid register
    let result = resolve_with_ext(&resolver, &key(' '), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::NotHandled));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert!(vim.pending_register.is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_count_flows_to_execute() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap_nf = MockKeymap::not_found();
    let input_nf = resolve_input(&keymap_nf);
    let mut extensions = ExtensionMap::new();

    // Accumulate count
    let _ = resolve_with_ext(&resolver, &key('4'), &mut state, &input_nf, &mut extensions);

    // Execute command
    let keymap_exec = MockKeymap::exact_only("cursor-down");
    let input_exec = resolve_input(&keymap_exec);

    let result = resolve_with_ext(&resolver, &key('j'), &mut state, &input_exec, &mut extensions);

    match result {
        ResolveResult::Execute(cmd, ctx) => {
            assert_eq!(cmd.name(), "cursor-down");
            assert_eq!(ctx.count, Some(4));
        }
        _ => panic!("Expected Execute, got {result:?}"),
    }
}

#[test]
fn test_ext_not_found_returns_not_handled() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('z'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_ext_prefix_only_returns_pending() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::prefix_only();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('g'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
fn test_ext_non_operator_exact_with_longer_returns_pending() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    // Create a non-operator ExactWithLonger (not enter-delete-operator)
    let keymap = MockKeymap::exact_with_longer("some-non-operator");
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('g'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_zero_not_count_without_pending() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("line-start");
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // 0 without pending count should be a command (line-start), not a count
    let result = resolve_with_ext(&resolver, &key('0'), &mut state, &input, &mut extensions);
    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "line-start");
        }
        _ => panic!("Expected Execute, got {result:?}"),
    }
}

#[test]
fn test_ext_zero_is_count_with_pending() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // First digit to start count
    let _ = resolve_with_ext(&resolver, &key('1'), &mut state, &input, &mut extensions);

    // Now 0 should be a count digit
    let result = resolve_with_ext(&resolver, &key('0'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_count, Some(10));
}

// ========================================================================
// Classify helpers tests
// ========================================================================

#[test]
fn test_classify_find_char_command_forward() {
    let cmd = CommandId::new(ModuleId::new("motions"), "find-char-forward");
    let result = VimNormalResolver::classify_find_char_command(&cmd);
    assert!(result.is_some());
}

#[test]
fn test_classify_find_char_command_backward() {
    let cmd = CommandId::new(ModuleId::new("motions"), "find-char-backward");
    let result = VimNormalResolver::classify_find_char_command(&cmd);
    assert!(result.is_some());
}

#[test]
fn test_classify_find_char_command_till_forward() {
    let cmd = CommandId::new(ModuleId::new("motions"), "till-char-forward");
    let result = VimNormalResolver::classify_find_char_command(&cmd);
    assert!(result.is_some());
}

#[test]
fn test_classify_find_char_command_till_backward() {
    let cmd = CommandId::new(ModuleId::new("motions"), "till-char-backward");
    let result = VimNormalResolver::classify_find_char_command(&cmd);
    assert!(result.is_some());
}

#[test]
fn test_classify_find_char_command_non_motion_module() {
    let cmd = CommandId::new(ModuleId::new("editor"), "find-char-forward");
    let result = VimNormalResolver::classify_find_char_command(&cmd);
    assert!(result.is_none());
}

#[test]
fn test_classify_find_char_command_unknown() {
    let cmd = CommandId::new(ModuleId::new("motions"), "cursor-down");
    let result = VimNormalResolver::classify_find_char_command(&cmd);
    assert!(result.is_none());
}

#[test]
fn test_classify_operator_mode_delete() {
    let result = VimNormalResolver::classify_operator_mode(&editor::ids::ENTER_DELETE_OPERATOR);
    assert_eq!(result, Some(VimMode::DELETE_ID));
}

#[test]
fn test_classify_operator_mode_yank() {
    let result = VimNormalResolver::classify_operator_mode(&editor::ids::ENTER_YANK_OPERATOR);
    assert_eq!(result, Some(VimMode::YANK_ID));
}

#[test]
fn test_classify_operator_mode_change() {
    let result = VimNormalResolver::classify_operator_mode(&editor::ids::ENTER_CHANGE_OPERATOR);
    assert_eq!(result, Some(VimMode::CHANGE_ID));
}

#[test]
fn test_classify_operator_mode_non_operator() {
    let cmd = CommandId::new(ModuleId::new("editor"), "cursor-down");
    let result = VimNormalResolver::classify_operator_mode(&cmd);
    assert!(result.is_none());
}

// ========================================================================
// Macro-related helper tests
// ========================================================================

#[test]
fn test_is_macro_record_key() {
    assert!(VimNormalResolver::is_macro_record_key(&key('q')));
    assert!(!VimNormalResolver::is_macro_record_key(&key('w')));
    assert!(!VimNormalResolver::is_macro_record_key(&key_with_mod('q', Modifiers::CTRL)));
}

#[test]
fn test_is_macro_play_key() {
    assert!(VimNormalResolver::is_macro_play_key(&key('@')));
    assert!(!VimNormalResolver::is_macro_play_key(&key('a')));
    assert!(!VimNormalResolver::is_macro_play_key(&key_with_mod('@', Modifiers::CTRL)));
}

#[test]
fn test_pending_macro_starts_none() {
    let resolver = VimNormalResolver::new();
    assert!(resolver.pending_macro_op().is_none());
}

#[test]
fn test_set_and_clear_pending_macro() {
    let resolver = VimNormalResolver::new();
    resolver.set_pending_macro(PendingMacroOp::StartRecording);
    assert_eq!(resolver.pending_macro_op(), Some(PendingMacroOp::StartRecording));

    resolver.clear_pending_macro();
    assert!(resolver.pending_macro_op().is_none());
}

#[test]
fn test_clear_state_clears_macro() {
    let resolver = VimNormalResolver::new();
    resolver.set_pending_macro(PendingMacroOp::PlayMacro);
    resolver.clear_state();
    assert!(resolver.pending_macro_op().is_none());
}

#[test]
fn test_register_special_chars_valid() {
    let resolver = VimNormalResolver::new();
    // Set sentinel
    *resolver.pending_register.write().unwrap() = Some('"');

    // Special register characters should be accepted
    for c in ['+', '-', '*', '/', '.', '%', '#', ':'] {
        *resolver.pending_register.write().unwrap() = Some('"');
        let result = resolver.handle_register_char(&key(c));
        assert!(matches!(result, ResolveResult::Pending), "register char '{c}' should be valid");
    }
}

#[test]
fn test_build_context_no_count_no_register() {
    let resolver = VimNormalResolver::new();
    let ctx = resolver.build_context(KeySequence::new());
    assert!(ctx.count.is_none());
    assert!(ctx.register.is_none());
}

#[test]
fn test_build_context_with_count() {
    let resolver = VimNormalResolver::new();
    resolver.accumulate_count(&key('7'));
    let ctx = resolver.build_context(KeySequence::new());
    assert_eq!(ctx.count, Some(7));
}

#[test]
fn test_build_context_with_register() {
    let resolver = VimNormalResolver::new();
    *resolver.pending_register.write().unwrap() = Some('z');
    let ctx = resolver.build_context(KeySequence::new());
    assert_eq!(ctx.register, Some('z'));
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
fn test_default_resolver() {
    let resolver = VimNormalResolver::default();
    assert_eq!(resolver.mode_id(), &VimMode::NORMAL_ID);
    assert!(resolver.pending_count().is_none());
}

#[test]
fn test_handle_register_char_non_char_key() {
    // handle_register_char with non-Char key (e.g., Enter) should cancel
    let resolver = VimNormalResolver::new();
    *resolver.pending_register.write().unwrap() = Some('"');

    let result = resolver.handle_register_char(&KeyEvent::new(KeyCode::Enter));
    assert!(matches!(result, ResolveResult::NotHandled));
    assert!(resolver.pending_register().is_none());
}

#[test]
fn test_build_context_ext_no_count_no_register() {
    let resolver = VimNormalResolver::new();
    let mut vim = VimSessionState::default();
    let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
    assert!(ctx.count.is_none());
    assert!(ctx.register.is_none());
}

#[test]
fn test_build_context_ext_with_count() {
    let resolver = VimNormalResolver::new();
    let mut vim = VimSessionState {
        pending_count: Some(4),
        ..VimSessionState::default()
    };
    let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
    assert_eq!(ctx.count, Some(4));
    assert!(vim.pending_count.is_none()); // consumed
}

#[test]
fn test_build_context_ext_with_register() {
    let resolver = VimNormalResolver::new();
    let mut vim = VimSessionState {
        pending_register: Some('z'),
        ..VimSessionState::default()
    };
    let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
    assert_eq!(ctx.register, Some('z'));
    assert!(vim.pending_register.is_none()); // consumed
}

#[test]
fn test_build_context_ext_sentinel_register_filtered() {
    // The sentinel '"' should NOT appear in context
    let resolver = VimNormalResolver::new();
    let mut vim = VimSessionState {
        pending_register: Some('"'),
        ..VimSessionState::default()
    };
    let ctx = resolver.build_context_ext(KeySequence::new(), &mut vim);
    assert!(ctx.register.is_none(), "sentinel should be filtered");
}

#[test]
fn test_ext_find_char_forward() {
    // Test find-char interception in resolve_with_extensions
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let motions_module = ModuleId::new("motions");
    let keymap = MockKeymap {
        response: KeyLookupState::ExactOnly(CommandId::new(motions_module, "find-char-forward")),
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Press 'f' - should intercept and set pending_char, return Pending
    let result = resolve_with_ext(&resolver, &key('f'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::Pending),
        "find-char should return Pending, got {result:?}"
    );

    // Check that pending_char was set
    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_char, Some(PendingCharOp::FindForward));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_find_char_completes_with_character() {
    // After f is pressed and pending_char is set, the next character should complete
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let motions_module = ModuleId::new("motions");
    let keymap = MockKeymap {
        response: KeyLookupState::ExactOnly(CommandId::new(motions_module, "find-char-forward")),
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Press 'f' to set pending_char
    let _ = resolve_with_ext(&resolver, &key('f'), &mut state, &input, &mut extensions);

    // Now press 'a' - should execute find-char with char='a'
    let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);

    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd.name(), "dispatch-find-char");
        assert_eq!(ctx.metadata.get("find_char"), Some(&ArgValue::Char('a')));
        assert_eq!(
            ctx.metadata.get("find_direction"),
            Some(&ArgValue::String("forward".to_string()))
        );
        assert_eq!(ctx.metadata.get("find_inclusive"), Some(&ArgValue::Bool(true)));
    } else {
        panic!("Expected Execute, got {result:?}");
    }
}

#[test]
fn test_ext_find_char_backward() {
    // Test find-char-backward interception
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let motions_module = ModuleId::new("motions");
    let keymap = MockKeymap {
        response: KeyLookupState::ExactOnly(CommandId::new(motions_module, "find-char-backward")),
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let _ = resolve_with_ext(&resolver, &key('F'), &mut state, &input, &mut extensions);

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_char, Some(PendingCharOp::FindBackward));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_till_char_forward() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let motions_module = ModuleId::new("motions");
    let keymap = MockKeymap {
        response: KeyLookupState::ExactOnly(CommandId::new(motions_module, "till-char-forward")),
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let _ = resolve_with_ext(&resolver, &key('t'), &mut state, &input, &mut extensions);

    // Complete with 'x'
    let result = resolve_with_ext(&resolver, &key('x'), &mut state, &input, &mut extensions);

    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd.name(), "dispatch-find-char");
        assert_eq!(ctx.metadata.get("find_char"), Some(&ArgValue::Char('x')));
        assert_eq!(
            ctx.metadata.get("find_direction"),
            Some(&ArgValue::String("forward".to_string()))
        );
        // till is NOT inclusive (is_find() returns false for Till)
        assert_eq!(ctx.metadata.get("find_inclusive"), Some(&ArgValue::Bool(false)));
    } else {
        panic!("Expected Execute, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_till_char_backward() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let motions_module = ModuleId::new("motions");
    let keymap = MockKeymap {
        response: KeyLookupState::ExactOnly(CommandId::new(motions_module, "till-char-backward")),
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let _ = resolve_with_ext(&resolver, &key('T'), &mut state, &input, &mut extensions);

    let result = resolve_with_ext(&resolver, &key('z'), &mut state, &input, &mut extensions);

    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd.name(), "dispatch-find-char");
        assert_eq!(
            ctx.metadata.get("find_direction"),
            Some(&ArgValue::String("backward".to_string()))
        );
    } else {
        panic!("Expected Execute, got {result:?}");
    }
}

// ========================================================================
// Replace char interception tests (#554)
// ========================================================================

#[test]
fn test_ext_replace_char_sets_pending() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let editor_module = reovim_kernel::api::v1::ModuleId::new("editor");
    let keymap = MockKeymap {
        response: KeyLookupState::ExactOnly(CommandId::new(editor_module, "replace-char-start")),
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('r'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Pending));

    let vim = extensions.get::<VimSessionState>().unwrap();
    assert_eq!(vim.pending_char, Some(PendingCharOp::Replace));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_replace_char_dispatches() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let editor_module = reovim_kernel::api::v1::ModuleId::new("editor");
    let keymap = MockKeymap {
        response: KeyLookupState::ExactOnly(CommandId::new(editor_module, "replace-char-start")),
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Press 'r' to set pending_char=Replace
    let _ = resolve_with_ext(&resolver, &key('r'), &mut state, &input, &mut extensions);

    // Now press 'x' - should dispatch replace-char with char='x'
    let result = resolve_with_ext(&resolver, &key('x'), &mut state, &input, &mut extensions);

    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd.name(), "replace-char");
        assert_eq!(cmd.module().as_str(), "editor");
        assert_eq!(ctx.metadata.get("replace_char"), Some(&ArgValue::Char('x')));
    } else {
        panic!("Expected Execute, got {result:?}");
    }
}

#[test]
fn test_ext_macro_record_start_and_stop() {
    use {reovim_kernel::api::v1::RwLock, reovim_types_text::RegisterBank, std::sync::Arc};
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::NORMAL_ID;

    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();

    let registers = Arc::new(RwLock::new(RegisterBank::new()));
    let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
    let mut extensions = ExtensionMap::new();

    // Press 'q' to start recording
    let result = resolve_with_ext(&resolver, &key('q'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::Pending),
        "q should return Pending (waiting for register), got {result:?}"
    );

    // Press 'a' to select register 'a'
    let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::Completed),
        "qa should start recording, got {result:?}"
    );

    // Verify recording started
    let vim = extensions.get::<VimSessionState>().unwrap();
    assert!(vim.is_recording());

    // Record some keys
    let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);
    // 'd' is NotFound, so it becomes NotHandled after being recorded
    assert!(matches!(result, ResolveResult::NotHandled));

    // Press 'q' again to stop recording
    let result = resolve_with_ext(&resolver, &key('q'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::Completed),
        "q should stop recording, got {result:?}"
    );

    // Verify recording stopped
    let vim = extensions.get::<VimSessionState>().unwrap();
    assert!(!vim.is_recording());
}

#[test]
fn test_ext_macro_record_invalid_register() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Press 'q' to start recording
    let _ = resolve_with_ext(&resolver, &key('q'), &mut state, &input, &mut extensions);

    // Press 'A' (uppercase, invalid register for macro recording)
    let result = resolve_with_ext(&resolver, &key('A'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::NotHandled),
        "Invalid register should return NotHandled, got {result:?}"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_macro_play_basic() {
    use {
        reovim_kernel::api::v1::RwLock,
        reovim_types_text::{RegisterBank, RegisterContent},
        std::sync::Arc,
    };
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::NORMAL_ID;

    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();

    let registers = Arc::new(RwLock::new(RegisterBank::new()));
    // Store a macro in register 'a'
    registers
        .write()
        .set_named('a', RegisterContent::characterwise("dw"));

    let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
    let mut extensions = ExtensionMap::new();

    // Press '@' to play macro
    let result = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::Pending),
        "@ should return Pending (waiting for register), got {result:?}"
    );

    // Press 'a' to select register
    let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
    if let ResolveResult::InjectKeys {
        keys,
        exit_macro_playback,
    } = result
    {
        assert!(!keys.is_empty());
        assert!(exit_macro_playback);
    } else {
        panic!("Expected InjectKeys, got {result:?}");
    }
}

#[test]
fn test_ext_macro_play_depth_exceeded() {
    use crate::session_state::MAX_MACRO_DEPTH;

    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Set depth to max
    {
        let vim = extensions.get_or_insert::<VimSessionState>();
        vim.macro_playback_depth = MAX_MACRO_DEPTH;
    }

    // Press '@' - should be blocked due to depth limit
    let result = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::NotHandled),
        "@ with exceeded depth should return NotHandled, got {result:?}"
    );
}

#[test]
fn test_ext_macro_play_invalid_register() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Press '@' to start playback
    let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);

    // Press '5' (not a valid lowercase register)
    let result = resolve_with_ext(&resolver, &key('5'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::NotHandled),
        "Invalid register for @ should return NotHandled, got {result:?}"
    );
}

#[test]
fn test_ext_macro_play_empty_register() {
    use {reovim_kernel::api::v1::RwLock, reovim_types_text::RegisterBank, std::sync::Arc};
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::NORMAL_ID;

    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();

    let registers = Arc::new(RwLock::new(RegisterBank::new()));
    // Register 'a' is empty (no content set)

    let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
    let mut extensions = ExtensionMap::new();

    // Press '@' then 'a'
    let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
    let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::NotHandled),
        "Empty register should return NotHandled, got {result:?}"
    );
}

#[test]
fn test_ext_macro_play_no_registers() {
    // When input.registers is None, macro playback should return NotHandled
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap); // no registers field
    let mut extensions = ExtensionMap::new();

    // Press '@' then 'a'
    let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
    let result = resolve_with_ext(&resolver, &key('a'), &mut state, &input, &mut extensions);
    assert!(
        matches!(result, ResolveResult::NotHandled),
        "No registers should return NotHandled, got {result:?}"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_macro_play_at_at_repeat() {
    use {
        reovim_kernel::api::v1::RwLock,
        reovim_types_text::{RegisterBank, RegisterContent},
        std::sync::Arc,
    };
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::NORMAL_ID;

    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();

    let registers = Arc::new(RwLock::new(RegisterBank::new()));
    registers
        .write()
        .set_named('b', RegisterContent::characterwise("j"));

    let input = ResolveInput::with_registers(&EMPTY_KEYS, &MODE, &keymap, &registers);
    let mut extensions = ExtensionMap::new();

    // First play @b
    let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
    let result = resolve_with_ext(&resolver, &key('b'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::InjectKeys { .. }));

    // Reset playback depth so we can repeat
    {
        let vim = extensions.get_mut::<VimSessionState>().unwrap();
        vim.macro_playback_depth = 0;
    }

    // Now @@ should repeat last macro (register 'b')
    let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
    let result = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);
    if let ResolveResult::InjectKeys { keys, .. } = result {
        assert!(!keys.is_empty());
    } else {
        panic!("Expected InjectKeys for @@, got {result:?}");
    }
}

#[test]
fn test_ext_macro_play_non_char_key() {
    // When pending PlayMacro and a non-Char key (like Enter) is pressed
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Press '@' to start playback
    let _ = resolve_with_ext(&resolver, &key('@'), &mut state, &input, &mut extensions);

    // Press Enter (non-Char key, invalid register)
    let result = resolve_with_ext(
        &resolver,
        &KeyEvent::new(KeyCode::Enter),
        &mut state,
        &input,
        &mut extensions,
    );
    assert!(
        matches!(result, ResolveResult::NotHandled),
        "Non-char key for @ should return NotHandled, got {result:?}"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_operator_exact_only_delete() {
    // Test operator interception via ExactOnly (not ExactWithLonger)
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::enter_delete_operator();
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);
    match result {
        ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
            assert_eq!(mode, VimMode::DELETE_ID);
        }
        _ => panic!("Expected ModeTransition::Push, got {result:?}"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_operator_exact_with_longer_delete() {
    // Test operator interception via ExactWithLonger
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::ExactWithLonger {
            exact: CommandId::new(EDITOR_MODULE, "enter-delete-operator"),
        },
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('d'), &mut state, &input, &mut extensions);
    match result {
        ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
            assert_eq!(mode, VimMode::DELETE_ID);
        }
        _ => panic!("Expected ModeTransition::Push, got {result:?}"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_operator_exact_with_longer_yank() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::ExactWithLonger {
            exact: CommandId::new(EDITOR_MODULE, "enter-yank-operator"),
        },
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('y'), &mut state, &input, &mut extensions);
    match result {
        ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
            assert_eq!(mode, VimMode::YANK_ID);
        }
        _ => panic!("Expected ModeTransition::Push, got {result:?}"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ext_operator_exact_with_longer_change() {
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::ExactWithLonger {
            exact: CommandId::new(EDITOR_MODULE, "enter-change-operator"),
        },
    };
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    let result = resolve_with_ext(&resolver, &key('c'), &mut state, &input, &mut extensions);
    match result {
        ResolveResult::ModeTransition(ModeTransition::Push { mode, .. }) => {
            assert_eq!(mode, VimMode::CHANGE_ID);
        }
        _ => panic!("Expected ModeTransition::Push, got {result:?}"),
    }
}

#[test]
fn test_ext_recording_records_keys() {
    // When recording, keys should be recorded in VimSessionState
    let resolver = VimNormalResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut extensions = ExtensionMap::new();

    // Start recording
    {
        let vim = extensions.get_or_insert::<VimSessionState>();
        vim.start_recording('a');
    }

    // Process a key while recording
    let result = resolve_with_ext(&resolver, &key('j'), &mut state, &input, &mut extensions);
    assert!(matches!(result, ResolveResult::Execute(..)));

    // Check the key was recorded
    let vim = extensions.get::<VimSessionState>().unwrap();
    assert!(!vim.recording_keys.is_empty());
}

#[test]
fn test_ext_register_char_ext_valid() {
    let resolver = VimNormalResolver::new();
    let mut vim = VimSessionState {
        pending_register: Some('"'),
        ..VimSessionState::default()
    };

    let result = resolver.handle_register_char_ext(&key('z'), &mut vim);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(vim.pending_register, Some('z'));
}

#[test]
fn test_ext_register_char_ext_invalid_non_char() {
    // handle_register_char_ext with non-Char key
    let resolver = VimNormalResolver::new();
    let mut vim = VimSessionState {
        pending_register: Some('"'),
        ..VimSessionState::default()
    };

    let result = resolver.handle_register_char_ext(&KeyEvent::new(KeyCode::Enter), &mut vim);
    assert!(matches!(result, ResolveResult::NotHandled));
    assert!(vim.pending_register.is_none());
}

#[test]
fn test_pending_keys_empty_initially() {
    let resolver = VimNormalResolver::new();
    assert!(resolver.pending_keys().is_empty());
}

#[test]
fn test_pending_keys_after_push() {
    let resolver = VimNormalResolver::new();
    resolver.push_pending_key(key('g'));
    let keys = resolver.pending_keys();
    assert_eq!(keys.len(), 1);
}
