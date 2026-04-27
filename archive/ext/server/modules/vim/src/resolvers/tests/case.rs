#![allow(clippy::doc_markdown, clippy::equatable_if_let)]

use {
    reovim_driver_text_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, KeymapQuery, ModeKeyResolver, ModeState,
        ModeTransition, PopResult, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
};

use {
    super::super::{case::*, operator_common::OperatorType},
    crate::modes::VimMode,
};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn test_state(mode: ModeId) -> ModeState {
    ModeState::new(mode)
}

/// Mock keymap that always returns `NotFound`.
struct NotFoundKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for NotFoundKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::NotFound
    }
}

const TEST_MODULE: ModuleId = ModuleId::new("test");

struct MockKeymap {
    response: KeyLookupState,
}

impl MockKeymap {
    fn exact_only(cmd: &'static str) -> Self {
        Self {
            response: KeyLookupState::ExactOnly(CommandId::new(TEST_MODULE, cmd)),
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

fn resolve_input_lowercase(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::LOWERCASE_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

fn resolve_input_uppercase(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::UPPERCASE_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

fn resolve_input_toggle(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::TOGGLE_CASE_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

// ============================================================================
// Construction and accessors
// ============================================================================

#[test]
fn new_lowercase_resolver() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    assert_eq!(resolver.mode_id(), &VimMode::LOWERCASE_ID);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
fn new_uppercase_resolver() {
    let resolver = VimCaseResolver::new(OperatorType::Uppercase);
    assert_eq!(resolver.mode_id(), &VimMode::UPPERCASE_ID);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
fn new_toggle_case_resolver() {
    let resolver = VimCaseResolver::new(OperatorType::ToggleCase);
    assert_eq!(resolver.mode_id(), &VimMode::TOGGLE_CASE_ID);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
fn pending_keys_initially_empty() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    assert!(resolver.pending_keys().is_empty());
}

#[test]
fn reset_clears_state() {
    let mut resolver = VimCaseResolver::new(OperatorType::Lowercase);
    // Accumulate some state via count digit
    {
        let mut state = resolver.state.write().expect("lock");
        state.accumulate_motion_count(&key('3'));
        state.push_key(key('w'));
    }
    assert!(!resolver.pending_keys().is_empty());

    resolver.reset();

    let state = resolver.state();
    assert_eq!(state.motion_count, None);
    assert!(resolver.pending_keys().is_empty());
}

// ============================================================================
// Escape handling
// ============================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn escape_cancels_lowercase() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_lowercase(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut mstate, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        assert!(matches!(r, PopResult::Cancelled));
    } else {
        panic!("expected Cancelled, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn ctrl_bracket_cancels() {
    use reovim_driver_text_input::Modifiers;
    let resolver = VimCaseResolver::new(OperatorType::Uppercase);
    let mut mstate = test_state(VimMode::UPPERCASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_uppercase(&keymap);

    let esc_key = KeyEvent::with_modifiers(KeyCode::Char('['), Modifiers::CTRL);
    let result = resolver.resolve_with_keymap(&esc_key, &mut mstate, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        assert!(matches!(r, PopResult::Cancelled));
    } else {
        panic!("expected Cancelled, got {result:?}");
    }
}

// ============================================================================
// Count digit accumulation
// ============================================================================

#[test]
fn count_digit_accumulates() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_lowercase(&keymap);

    let result = resolver.resolve_with_keymap(&key('3'), &mut mstate, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(3));

    let result = resolver.resolve_with_keymap(&key('5'), &mut mstate, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(35));
}

#[test]
fn count_with_zero_after_digit() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_lowercase(&keymap);

    let result = resolver.resolve_with_keymap(&key('2'), &mut mstate, &input);
    assert!(matches!(result, ResolveResult::Pending));

    let result = resolver.resolve_with_keymap(&key('0'), &mut mstate, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(20));
}

// ============================================================================
// Line operator (guu, gUU, g~~)
// ============================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn line_operator_guu() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_lowercase(&keymap);

    let result = resolver.resolve_with_keymap(&key('u'), &mut mstate, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_subsys_command_types::ArgValue::Bool(true))
            );
            assert_eq!(args.get("count"), Some(&reovim_subsys_command_types::ArgValue::Count(1)));
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(non_snake_case)]
fn line_operator_gUU() {
    let resolver = VimCaseResolver::new(OperatorType::Uppercase);
    let mut mstate = test_state(VimMode::UPPERCASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_uppercase(&keymap);

    let result = resolver.resolve_with_keymap(&key('U'), &mut mstate, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_subsys_command_types::ArgValue::Bool(true))
            );
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn line_operator_toggle_case() {
    let resolver = VimCaseResolver::new(OperatorType::ToggleCase);
    let mut mstate = test_state(VimMode::TOGGLE_CASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_toggle(&keymap);

    let result = resolver.resolve_with_keymap(&key('~'), &mut mstate, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_subsys_command_types::ArgValue::Bool(true))
            );
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn line_operator_with_motion_count() {
    // Simulate gu3u (motion count 3, then line operator)
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = NotFoundKeymap;
    let input = resolve_input_lowercase(&keymap);

    // Type "3" (motion count)
    let result = resolver.resolve_with_keymap(&key('3'), &mut mstate, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(3));

    // Type "u" (line operator)
    let result = resolver.resolve_with_keymap(&key('u'), &mut mstate, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_subsys_command_types::ArgValue::Bool(true))
            );
            // count = operator_count(1) * motion_count(3) = 3
            assert_eq!(args.get("count"), Some(&reovim_subsys_command_types::ArgValue::Count(3)));
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

// ============================================================================
// Keymap policy: Execute (motion found)
// ============================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn motion_key_executes() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = MockKeymap::exact_only("word-forward");
    let input = resolve_input_lowercase(&keymap);

    let result = resolver.resolve_with_keymap(&key('w'), &mut mstate, &input);

    if let ResolveResult::Execute(cmd, _ctx) = result {
        assert_eq!(cmd.name(), "word-forward");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

// ============================================================================
// Keymap policy: Pending (prefix match)
// ============================================================================

#[test]
fn prefix_match_returns_pending() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = MockKeymap::prefix_only();
    let input = resolve_input_lowercase(&keymap);

    let result = resolver.resolve_with_keymap(&key('g'), &mut mstate, &input);
    assert!(matches!(result, ResolveResult::Pending), "PrefixOnly should return Pending");
}

// ============================================================================
// Keymap policy: Cancel (not found)
// ============================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn not_found_cancels() {
    let resolver = VimCaseResolver::new(OperatorType::Uppercase);
    let mut mstate = test_state(VimMode::UPPERCASE_ID);
    let keymap = MockKeymap::not_found();
    let input = resolve_input_uppercase(&keymap);

    let result = resolver.resolve_with_keymap(&key('z'), &mut mstate, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        assert!(matches!(r, PopResult::Cancelled));
    } else {
        panic!("expected Cancelled, got {result:?}");
    }
}

// ============================================================================
// Execute with count
// ============================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_motion_with_count() {
    let resolver = VimCaseResolver::new(OperatorType::Lowercase);
    // Set operator count via direct state manipulation
    {
        let mut state = resolver.state.write().expect("lock");
        state.operator_count = Some(2);
        state.initialized = true;
    }
    let mut mstate = test_state(VimMode::LOWERCASE_ID);
    let keymap = MockKeymap::exact_only("word-forward");
    let input = resolve_input_lowercase(&keymap);

    // Type "3" then "w"
    let result = resolver.resolve_with_keymap(&key('3'), &mut mstate, &input);
    assert!(matches!(result, ResolveResult::Pending));

    let result = resolver.resolve_with_keymap(&key('w'), &mut mstate, &input);

    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd.name(), "word-forward");
        // explicit_count = operator(2) * motion(3) = 6
        assert_eq!(ctx.count, Some(6));
    } else {
        panic!("expected Execute, got {result:?}");
    }
}
