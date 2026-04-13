#![allow(clippy::doc_markdown)]

use {
    reovim_domain_text::Position,
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, KeymapQuery, ModeKeyResolver,
        ModeState, Modifiers, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::{BufferId, CommandId, ModeId, WindowId},
};

use {super::super::commandline::*, crate::modes::VimMode};

// =========================================================================
// MockSession for resolve_with_session tests
// =========================================================================

use {
    reovim_domain_text::{Edit, UndoResult},
    reovim_driver_session::{
        Selection, WindowError,
        api::{
            BufferApi, ChangeTracker, CommandApi, ModeApi, ModeError, StateChanges, UndoApi,
            WindowApi,
        },
    },
    reovim_subsys_command_types::{CommandContext, CommandResult},
};

/// Minimal mock implementing `SessionApiDyn` for resolve_with_session tests.
/// The commandline resolver does not use the session, so this mock is a no-op.
struct MockSession {
    mode: ModeId,
}

impl MockSession {
    fn new() -> Self {
        Self {
            mode: VimMode::COMMANDLINE_ID,
        }
    }
}

impl ModeApi for MockSession {
    fn current_mode(&self) -> &ModeId {
        &self.mode
    }
    fn home_mode(&self) -> &ModeId {
        &self.mode
    }
    fn mode_depth(&self) -> usize {
        1
    }
    fn is_mode_active(&self, _mode: &ModeId) -> bool {
        false
    }
    fn mode_stack(&self) -> Vec<ModeId> {
        vec![self.mode.clone()]
    }
    fn push_mode(&mut self, _mode: ModeId, _ctx: reovim_driver_session::TransitionContext) {}
    fn pop_mode(
        &mut self,
        _result: Option<reovim_driver_session::PopResult>,
    ) -> Result<(), ModeError> {
        Ok(())
    }
    fn set_mode(&mut self, mode: ModeId, _ctx: reovim_driver_session::TransitionContext) {
        self.mode = mode;
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl BufferApi for MockSession {
    fn active_buffer(&self) -> Option<BufferId> {
        None
    }
    fn set_active_buffer(&mut self, _id: Option<BufferId>) {}
    fn buffer_line(&self, _b: BufferId, _l: usize) -> Option<String> {
        None
    }
    fn buffer_line_count(&self, _b: BufferId) -> Option<usize> {
        None
    }
    fn buffer_line_len(&self, _b: BufferId, _l: usize) -> Option<usize> {
        None
    }
    fn buffer_text_range(&self, _b: BufferId, _s: Position, _e: Position) -> Option<String> {
        None
    }
    fn buffer_content(&self, _b: BufferId) -> Option<String> {
        None
    }
    fn buffer_file_path(&self, _b: BufferId) -> Option<String> {
        None
    }
    fn is_buffer_modified(&self, _b: BufferId) -> Option<bool> {
        None
    }
    fn set_buffer_modified(&mut self, _b: BufferId, _m: bool) {}
    fn insert_text(&mut self, _b: BufferId, _p: Position, _t: &str) {}
    fn delete_range(&mut self, _b: BufferId, _s: Position, _e: Position) {}
    fn create_buffer(&mut self, _n: Option<&str>, _c: &str) -> BufferId {
        BufferId::new()
    }
    fn delete_buffer(
        &mut self,
        _b: BufferId,
    ) -> Result<(), reovim_driver_session::api::BufferError> {
        Ok(())
    }
    fn rename_buffer(&mut self, _b: BufferId, _n: &str) {}
    fn replace_content(&mut self, _b: BufferId, _c: &str) {}
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl WindowApi for MockSession {
    fn active_window(&self) -> Option<WindowId> {
        None
    }
    fn cursor_position(&self) -> Option<Position> {
        None
    }
    fn window_count(&self) -> usize {
        0
    }
    fn window_buffer(&self, _w: WindowId) -> Option<BufferId> {
        None
    }
    fn create_window(&mut self, _b: Option<BufferId>) -> WindowId {
        WindowId::new()
    }
    fn close_window(&mut self, _w: WindowId) -> Result<(), WindowError> {
        Ok(())
    }
    fn focus_window(&mut self, _w: WindowId) -> Result<(), WindowError> {
        Ok(())
    }
    fn set_window_buffer(&mut self, _w: WindowId, _b: BufferId) -> Result<(), WindowError> {
        Ok(())
    }
    fn set_active_selection(&mut self, _selection: Option<Selection>) {}
    fn active_selection(&self) -> Option<&Selection> {
        None
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandApi for MockSession {
    fn execute_command(&mut self, _cmd: CommandId, _ctx: CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl UndoApi for MockSession {
    fn undo(&mut self, _b: BufferId) -> Option<UndoResult> {
        None
    }
    fn redo(&mut self, _b: BufferId) -> Option<UndoResult> {
        None
    }
    fn record_edit(&mut self, _b: BufferId, _e: Vec<Edit>, _cb: Position, _ca: Position) {}
    fn can_undo(&self, _b: BufferId) -> bool {
        false
    }
    fn can_redo(&self, _b: BufferId) -> bool {
        false
    }
    fn undo_mine(&mut self, _b: BufferId) -> Option<UndoResult> {
        None
    }
    fn redo_mine(&mut self, _b: BufferId) -> Option<UndoResult> {
        None
    }
    fn record_edit_mine(&mut self, _b: BufferId, _e: Vec<Edit>, _cb: Position, _ca: Position) {}
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ChangeTracker for MockSession {
    fn take_changes(&mut self) -> StateChanges {
        StateChanges::new()
    }
    fn record_cursor_move(&mut self, _b: BufferId) {}
    fn record_selection_change(&mut self, _b: BufferId) {}
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn key_with_mod(c: char, modifiers: Modifiers) -> KeyEvent {
    KeyEvent::with_modifiers(KeyCode::Char(c), modifiers)
}

fn test_state() -> ModeState {
    ModeState::new(VimMode::COMMANDLINE_ID)
}

/// Mock keymap that always returns `NotFound` (no bindings).
struct NotFoundKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for NotFoundKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::NotFound
    }
}

fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::COMMANDLINE_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

#[test]
fn test_new_resolver() {
    let resolver = VimCommandLineResolver::new();
    assert_eq!(resolver.mode_id(), &VimMode::COMMANDLINE_ID);
}

#[test]
fn test_insert_character() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    // Command-line mode routes chars to CmdlineState extension via resolve_with_session
    let mut session = MockSession::new();
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('w'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));

    let result = resolver.resolve_with_session(
        &key('q'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));

    let result = resolver.resolve_with_session(
        &key(' '),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));
}

#[test]
fn test_escape_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_enter_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_backspace_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result =
        resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Backspace), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_ctrl_char_not_inserted() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result =
        resolver.resolve_with_keymap(&key_with_mod('c', Modifiers::CTRL), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_mode_id() {
    let resolver = VimCommandLineResolver::new();
    assert_eq!(resolver.mode_id().name(), "command");
}

#[test]
fn test_inherits_from() {
    let resolver = VimCommandLineResolver::new();
    assert!(resolver.inherits_from().is_none());
}

// ========================================================================
// Additional Command-Line Resolver Tests
// ========================================================================

#[test]
fn test_default_impl() {
    let resolver = VimCommandLineResolver::default();
    assert_eq!(resolver.mode_id(), &VimMode::COMMANDLINE_ID);
}

#[test]
fn test_reset_is_noop() {
    let mut resolver = VimCommandLineResolver::new();
    resolver.reset();
    assert_eq!(resolver.mode_id(), &VimMode::COMMANDLINE_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_digits() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::new();
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    for c in '0'..='9' {
        let result = resolver.resolve_with_session(
            &key(c),
            &mut state,
            &input,
            &mut session,
            &mut shared,
            &mut client,
        );
        assert!(
            matches!(result, ResolveResult::Completed),
            "digit '{c}' should be insertable (Completed)"
        );
    }
}

#[test]
fn test_insert_special_symbols() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::new();
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('/'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));

    let result = resolver.resolve_with_session(
        &key('!'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));

    let result = resolver.resolve_with_session(
        &key('.'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));
}

#[test]
fn test_alt_char_not_inserted() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result =
        resolver.resolve_with_keymap(&key_with_mod('a', Modifiers::ALT), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_tab_not_inserted() {
    // Tab is not insertable in command-line mode (might be for completion)
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Tab), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_arrow_keys_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Up), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Down), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

/// Mock keymap returning ExactOnly for testing keymap interaction.
struct ExactKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for ExactKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::ExactOnly(reovim_kernel::api::v1::CommandId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "test-cmd",
        ))
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_escape_executes_with_keymap_binding() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = ExactKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "test-cmd");
        }
        _ => panic!("expected Execute, got {result:?}"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_enter_executes_with_keymap_binding() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = ExactKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "test-cmd");
        }
        _ => panic!("expected Execute, got {result:?}"),
    }
}

/// Mock keymap returning PrefixOnly for testing.
struct PrefixKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for PrefixKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::PrefixOnly
    }
}

#[test]
fn test_prefix_only_returns_pending() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = PrefixKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
fn test_delete_key_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Delete), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_mode_id_module() {
    let resolver = VimCommandLineResolver::new();
    assert_eq!(resolver.mode_id().module().as_str(), "vim");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_uppercase_chars() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::new();
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    for c in 'A'..='Z' {
        let result = resolver.resolve_with_session(
            &key(c),
            &mut state,
            &input,
            &mut session,
            &mut shared,
            &mut client,
        );
        assert!(
            matches!(result, ResolveResult::Completed),
            "uppercase '{c}' should be insertable (Completed)"
        );
    }
}

#[test]
fn test_insert_unicode_char() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::new();
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Char('\u{00e9}')),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));
}

#[test]
fn test_ctrl_shift_not_inserted() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    // Ctrl+Shift combination should not insert
    let result = resolver.resolve_with_keymap(
        &key_with_mod('a', Modifiers::CTRL | Modifiers::SHIFT),
        &mut state,
        &input,
    );
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_home_key_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Home), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_end_key_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::End), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

/// Mock keymap returning `ExactWithLonger`.
struct ExactWithLongerKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for ExactWithLongerKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::ExactWithLonger {
            exact: reovim_kernel::api::v1::CommandId::new(
                reovim_kernel::api::v1::ModuleId::new("test"),
                "test-cmd",
            ),
        }
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_exact_with_longer_executes() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = ExactWithLongerKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "test-cmd");
        }
        _ => panic!("expected Execute, got {result:?}"),
    }
}

#[test]
fn test_const_new() {
    const RESOLVER: VimCommandLineResolver = VimCommandLineResolver::new();
    assert_eq!(RESOLVER.mode_id().name(), "command");
}

#[test]
fn test_shift_char_is_insertable() {
    // Shift+char (like uppercase) should still be insertable
    // because the resulting KeyCode is Char with the uppercase char
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::new();
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key_with_mod('A', Modifiers::SHIFT),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));
}

#[test]
fn test_reset_then_resolve() {
    let mut resolver = VimCommandLineResolver::new();
    resolver.reset();

    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::new();
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('a'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );
    assert!(matches!(result, ResolveResult::Completed));
}

#[test]
fn test_function_keys_not_handled() {
    let resolver = VimCommandLineResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    for code in [KeyCode::F(1), KeyCode::F(5), KeyCode::F(12)] {
        let result = resolver.resolve_with_keymap(&KeyEvent::new(code), &mut state, &input);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "Function key {code:?} should not be handled"
        );
    }
}
