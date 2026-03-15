#![allow(clippy::doc_markdown, clippy::significant_drop_tightening)]

use {
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, KeymapQuery,
        ModeKeyResolver, ModeState, Modifiers, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::{
        BufferId, CommandId, ModeId, ModuleId, Position, WindowId,
    },
};

use {
    super::super::replace::*,
    crate::{VimSessionState, modes::VimMode},
};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn test_state() -> ModeState {
    ModeState::new(VimMode::REPLACE_ID)
}

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
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for MockKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        self.response.clone()
    }
}

fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::REPLACE_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

// =========================================================================
// MockSession for resolve_with_session tests
// =========================================================================

use {
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{
        Selection, WindowError,
        api::{
            BufferApi, ChangeTracker, CommandApi, ModeApi, ModeError, StateChanges, UndoApi,
            WindowApi,
        },
    },
    reovim_kernel::api::v1::{Edit, UndoResult},
};

/// Mock session with configurable buffer content.
struct MockSession {
    mode: ModeId,
    cursor: Option<Position>,
    buffer_id: Option<BufferId>,
    lines: Vec<String>,
}

impl MockSession {
    fn with_content(content: &str) -> Self {
        let buffer_id = BufferId::new();
        let lines: Vec<String> = content.lines().map(String::from).collect();
        Self {
            mode: VimMode::REPLACE_ID,
            cursor: Some(Position::new(0, 0)),
            buffer_id: Some(buffer_id),
            lines: if lines.is_empty() { vec![String::new()] } else { lines },
        }
    }

    fn with_cursor(mut self, line: usize, col: usize) -> Self {
        self.cursor = Some(Position::new(line, col));
        self
    }
}

impl ModeApi for MockSession {
    fn current_mode(&self) -> &ModeId { &self.mode }
    fn home_mode(&self) -> &ModeId { &self.mode }
    fn mode_depth(&self) -> usize { 1 }
    fn is_mode_active(&self, _mode: &ModeId) -> bool { false }
    fn mode_stack(&self) -> Vec<ModeId> { vec![self.mode.clone()] }
    fn push_mode(&mut self, _mode: ModeId, _ctx: reovim_driver_session::TransitionContext) {}
    fn pop_mode(&mut self, _result: Option<reovim_driver_session::PopResult>) -> Result<(), ModeError> { Ok(()) }
    fn set_mode(&mut self, mode: ModeId, _ctx: reovim_driver_session::TransitionContext) { self.mode = mode; }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl BufferApi for MockSession {
    fn active_buffer(&self) -> Option<BufferId> { self.buffer_id }
    fn set_active_buffer(&mut self, _id: Option<BufferId>) {}
    fn buffer_line(&self, _b: BufferId, l: usize) -> Option<String> {
        self.lines.get(l).cloned()
    }
    fn buffer_line_count(&self, _b: BufferId) -> Option<usize> {
        Some(self.lines.len())
    }
    fn buffer_line_len(&self, _b: BufferId, l: usize) -> Option<usize> {
        self.lines.get(l).map(String::len)
    }
    fn buffer_text_range(&self, _b: BufferId, _s: Position, _e: Position) -> Option<String> { None }
    fn buffer_content(&self, _b: BufferId) -> Option<String> { None }
    fn buffer_file_path(&self, _b: BufferId) -> Option<String> { None }
    fn is_buffer_modified(&self, _b: BufferId) -> Option<bool> { None }
    fn set_buffer_modified(&mut self, _b: BufferId, _m: bool) {}
    fn insert_text(&mut self, _b: BufferId, _p: Position, _t: &str) {}
    fn delete_range(&mut self, _b: BufferId, _s: Position, _e: Position) {}
    fn create_buffer(&mut self, _n: Option<&str>, _c: &str) -> BufferId { BufferId::new() }
    fn delete_buffer(&mut self, _b: BufferId) -> Result<(), reovim_driver_session::api::BufferError> { Ok(()) }
    fn rename_buffer(&mut self, _b: BufferId, _n: &str) {}
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl WindowApi for MockSession {
    fn active_window(&self) -> Option<WindowId> { Some(WindowId::new()) }
    fn cursor_position(&self) -> Option<Position> { self.cursor }
    fn window_count(&self) -> usize { 1 }
    fn window_buffer(&self, _w: WindowId) -> Option<BufferId> { self.buffer_id }
    fn create_window(&mut self, _b: Option<BufferId>) -> WindowId { WindowId::new() }
    fn close_window(&mut self, _w: WindowId) -> Result<(), WindowError> { Ok(()) }
    fn focus_window(&mut self, _w: WindowId) -> Result<(), WindowError> { Ok(()) }
    fn set_window_buffer(&mut self, _w: WindowId, _b: BufferId) -> Result<(), WindowError> { Ok(()) }
    fn set_active_selection(&mut self, _selection: Option<Selection>) {}
    fn active_selection(&self) -> Option<&Selection> { None }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandApi for MockSession {
    fn execute_command(&mut self, _cmd: CommandId, _ctx: CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl UndoApi for MockSession {
    fn undo(&mut self, _b: BufferId) -> Option<UndoResult> { None }
    fn redo(&mut self, _b: BufferId) -> Option<UndoResult> { None }
    fn record_edit(&mut self, _b: BufferId, _e: Vec<Edit>, _cb: Position, _ca: Position) {}
    fn can_undo(&self, _b: BufferId) -> bool { false }
    fn can_redo(&self, _b: BufferId) -> bool { false }
    fn undo_mine(&mut self, _b: BufferId) -> Option<UndoResult> { None }
    fn redo_mine(&mut self, _b: BufferId) -> Option<UndoResult> { None }
    fn record_edit_mine(&mut self, _b: BufferId, _e: Vec<Edit>, _cb: Position, _ca: Position) {}
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ChangeTracker for MockSession {
    fn take_changes(&mut self) -> StateChanges { StateChanges::new() }
    fn record_cursor_move(&mut self, _b: BufferId) {}
    fn record_selection_change(&mut self, _b: BufferId) {}
}

// =========================================================================
// Basic Resolver Properties
// =========================================================================

#[test]
fn test_new_resolver() {
    let resolver = VimReplaceResolver::new();
    assert_eq!(resolver.mode_id(), &VimMode::REPLACE_ID);
}

#[test]
fn test_default_resolver() {
    let resolver = VimReplaceResolver::default();
    assert_eq!(resolver.mode_id(), &VimMode::REPLACE_ID);
}

#[test]
fn test_inherits_from_insert() {
    let resolver = VimReplaceResolver::new();
    assert_eq!(resolver.inherits_from(), Some(&VimMode::INSERT_ID));
}

#[test]
fn test_reset_is_noop() {
    let mut resolver = VimReplaceResolver::new();
    resolver.reset();
    assert_eq!(resolver.mode_id(), &VimMode::REPLACE_ID);
}

// =========================================================================
// Character Replacement
// =========================================================================

#[test]
fn test_insertable_char_returns_insert_char() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &key('X'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    // Should return InsertChar (after deleting the char under cursor)
    assert!(matches!(result, ResolveResult::InsertChar { char: 'X', .. }));
}

#[test]
fn test_replace_pushes_restore_entry() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    resolver.resolve_with_session(
        &key('X'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    let vim = client.get::<VimSessionState>().unwrap();
    assert_eq!(vim.replace_restore_stack.len(), 1);
    assert_eq!(vim.replace_restore_stack[0].original, Some('h'));
    assert_eq!(vim.replace_restore_stack[0].position, Position::new(0, 0));
}

#[test]
fn test_replace_at_eol_pushes_none_original() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hi").with_cursor(0, 2);
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &key('X'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::InsertChar { char: 'X', .. }));
    let vim = client.get::<VimSessionState>().unwrap();
    assert_eq!(vim.replace_restore_stack.len(), 1);
    assert!(vim.replace_restore_stack[0].original.is_none());
}

#[test]
fn test_replace_tracks_insert_buffer() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    resolver.resolve_with_session(
        &key('a'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    let vim = client.get::<VimSessionState>().unwrap();
    assert_eq!(vim.insert_buffer, "a");
}

// =========================================================================
// Newline in Replace Mode
// =========================================================================

#[test]
fn test_enter_returns_insert_char_newline() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Enter),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::InsertChar { char: '\n', .. }));

    // Should push a restore entry
    let vim = client.get::<VimSessionState>().unwrap();
    assert_eq!(vim.replace_restore_stack.len(), 1);
}

// =========================================================================
// Backspace
// =========================================================================

#[test]
fn test_backspace_dispatches_replace_backspace_command() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Backspace),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    // Should dispatch REPLACE_BACKSPACE command
    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "replace-backspace");
        }
        _ => panic!("expected Execute(replace-backspace), got {result:?}"),
    }
}

#[test]
fn test_ctrl_h_dispatches_replace_backspace() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &KeyEvent::with_modifiers(KeyCode::Char('h'), Modifiers::CTRL),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "replace-backspace");
        }
        _ => panic!("expected Execute(replace-backspace), got {result:?}"),
    }
}

// =========================================================================
// Non-insertable Keys (Escape, Arrows, etc.)
// =========================================================================

#[test]
fn test_escape_not_handled_delegates_to_keymap() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Escape),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_escape_with_keymap_binding_executes() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("exit-insert");
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Escape),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "exit-insert");
        }
        _ => panic!("expected Execute, got {result:?}"),
    }
}

#[test]
fn test_arrow_keys_not_handled() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Left),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::NotHandled));
}

// =========================================================================
// Tab in Replace Mode
// =========================================================================

#[test]
fn test_tab_replaces_char() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Tab),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::InsertChar { char: '\t', .. }));
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_no_buffer_returns_insert_char() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    session.buffer_id = None; // No buffer
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &key('X'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    // Should still return InsertChar even without buffer
    assert!(matches!(result, ResolveResult::InsertChar { char: 'X', .. }));
}

#[test]
fn test_no_cursor_returns_insert_char() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    session.cursor = None; // No cursor
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    let result = resolver.resolve_with_session(
        &key('X'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::InsertChar { char: 'X', .. }));
}

#[test]
fn test_ctrl_not_insertable() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    // Ctrl+A should NOT insert
    let result = resolver.resolve_with_session(
        &KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_alt_not_insertable() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::ALT),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::NotHandled));
}

// =========================================================================
// Prefix keymap
// =========================================================================

struct PrefixKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for PrefixKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::PrefixOnly
    }
}

#[test]
fn test_prefix_only_returns_pending() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = PrefixKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Escape),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    assert!(matches!(result, ResolveResult::Pending));
}

// =========================================================================
// ExactWithLonger keymap
// =========================================================================

struct ExactWithLongerKeymap;

#[cfg_attr(coverage_nightly, coverage(off))]
impl KeymapQuery for ExactWithLongerKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        KeyLookupState::ExactWithLonger {
            exact: CommandId::new(TEST_MODULE, "longer-cmd"),
        }
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_exact_with_longer_executes_exact() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = ExactWithLongerKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Escape),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    match result {
        ResolveResult::Execute(cmd, _) => {
            assert_eq!(cmd.name(), "longer-cmd");
        }
        _ => panic!("expected Execute, got {result:?}"),
    }
}

// =========================================================================
// Multiple replacements
// =========================================================================

#[test]
fn test_multiple_replacements_build_stack() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let _ = client.get_or_insert::<VimSessionState>();

    // Replace first char
    resolver.resolve_with_session(
        &key('A'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    // Replace second char (cursor would be at col 1 after runner handles InsertChar)
    session.cursor = Some(Position::new(0, 1));
    resolver.resolve_with_session(
        &key('B'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    let vim = client.get::<VimSessionState>().unwrap();
    assert_eq!(vim.replace_restore_stack.len(), 2);
    assert_eq!(vim.replace_restore_stack[0].original, Some('h'));
    assert_eq!(vim.replace_restore_stack[1].original, Some('e'));
    assert_eq!(vim.insert_buffer, "AB");
}

// =========================================================================
// Dot repeat recording
// =========================================================================

#[test]
fn test_records_repeat_key() {
    let resolver = VimReplaceResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let mut session = MockSession::with_content("hello");
    let mut shared = ExtensionMap::new();
    let mut client = ExtensionMap::new();
    let vim = client.get_or_insert::<VimSessionState>();
    vim.start_repeat_recording();

    resolver.resolve_with_session(
        &key('X'),
        &mut state,
        &input,
        &mut session,
        &mut shared,
        &mut client,
    );

    let vim = client.get::<VimSessionState>().unwrap();
    assert_eq!(vim.repeat_keys.len(), 1);
}
