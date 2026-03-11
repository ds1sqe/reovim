#![allow(clippy::doc_markdown, clippy::equatable_if_let)]

use {
    super::super::yank::*,
    crate::{modes::VimMode, session_state::PendingMotion},
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, KeymapQuery, ModeKeyResolver,
        ModeState, ModeTransition, PopResult, ResolveInput, ResolveResult,
    },
    reovim_driver_session::OperatorPendingState,
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId, Position},
};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn test_state() -> ModeState {
    ModeState::new(VimMode::YANK_ID)
}

/// Mock keymap that always returns `NotFound` (no bindings).
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
    static MODE: ModeId = VimMode::YANK_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

#[test]
fn test_new_resolver() {
    let resolver = VimYankResolver::new();
    assert_eq!(resolver.mode_id(), &VimMode::YANK_ID);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_escape_cancels() {
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        assert!(matches!(r, PopResult::Cancelled));
    } else {
        panic!("expected ModeTransition::Pop with Cancelled");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_line_operator_yy() {
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('y'), &mut state, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_driver_command_types::ArgValue::Bool(true))
            );
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

#[test]
fn test_count_digit() {
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('2'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(2));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_keymap_executes_motion() {
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("word-forward");
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);

    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "word-forward");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
fn test_default_resolver() {
    let resolver = VimYankResolver::default();
    assert_eq!(resolver.mode_id(), &VimMode::YANK_ID);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
fn test_with_context() {
    let resolver = VimYankResolver::with_context(Some(5), Some('b'));
    let state = resolver.state();
    assert_eq!(state.operator_count, Some(5));
    assert_eq!(state.register, Some('b'));
}

#[test]
fn test_with_context_none() {
    let resolver = VimYankResolver::with_context(None, None);
    let state = resolver.state();
    assert_eq!(state.operator_count, None);
    assert_eq!(state.register, None);
}

#[test]
fn test_multi_digit_count() {
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    // Type "12"
    let result = resolver.resolve_with_keymap(&key('1'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(1));

    let result = resolver.resolve_with_keymap(&key('2'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(12));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_line_operator_yy_with_count() {
    let resolver = VimYankResolver::with_context(Some(3), None);
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('y'), &mut state, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_driver_command_types::ArgValue::Bool(true))
            );
            // count should be 3 * 1 = 3
            assert_eq!(args.get("count"), Some(&reovim_driver_command_types::ArgValue::Count(3)));
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

#[test]
fn test_ctrl_c_cancels() {
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(
        &KeyEvent::with_modifiers(KeyCode::Char('c'), reovim_driver_input::Modifiers::CTRL),
        &mut state,
        &input,
    );

    // Ctrl-C is not Escape, so it will be treated as a key push, then looked up
    // and not found, resulting in cancel
    assert!(matches!(result, ResolveResult::ModeTransition(ModeTransition::Pop { .. })));
}

#[test]
fn test_reset_clears_state() {
    let mut resolver = VimYankResolver::with_context(Some(5), Some('a'));

    // Verify state is set
    let state = resolver.state();
    assert_eq!(state.operator_count, Some(5));

    // Reset
    resolver.reset();

    // State should be cleared
    let state = resolver.state();
    assert_eq!(state.operator_count, None);
    assert_eq!(state.register, None);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_unknown_key_cancels() {
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::NotFound,
    };
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        assert!(matches!(r, PopResult::Cancelled));
    } else {
        panic!("expected Cancelled, got {result:?}");
    }
}

// ========================================================================
// resolve_with_session tests
// ========================================================================

use {
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{
        TextObjRange, WindowError,
        api::{
            BufferApi, ChangeTracker, CommandApi, ModeApi, ModeError, StateChanges, UndoApi,
            WindowApi,
        },
    },
    reovim_kernel::api::v1::{BufferId, Edit, UndoResult, WindowId},
};

/// Minimal mock implementing `SessionApiDyn` for resolve_with_session tests.
struct MockSession {
    mode: ModeId,
    cursor: Option<Position>,
}

impl MockSession {
    fn new() -> Self {
        Self {
            mode: VimMode::YANK_ID,
            cursor: Some(Position::new(0, 0)),
        }
    }

    fn with_cursor(line: usize, col: usize) -> Self {
        Self {
            mode: VimMode::YANK_ID,
            cursor: Some(Position::new(line, col)),
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
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl WindowApi for MockSession {
    fn active_window(&self) -> Option<WindowId> {
        Some(WindowId::new())
    }
    fn cursor_position(&self) -> Option<Position> {
        self.cursor
    }
    fn window_count(&self) -> usize {
        1
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
    fn set_active_selection(&mut self, _selection: Option<reovim_driver_session::Selection>) {}
    fn active_selection(&self) -> Option<&reovim_driver_session::Selection> {
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

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_escape_cancels() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::new(KeyCode::Escape),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        assert!(matches!(r, PopResult::Cancelled));
    } else {
        panic!("expected Cancelled, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_initializes_from_vim_state() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(5, 3);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // Pre-set pending count and register in VimSessionState
    {
        let vim = extensions.get_or_insert::<crate::VimSessionState>();
        vim.pending_count = Some(4);
        vim.pending_register = Some('z');
    }

    let result = resolver.resolve_with_session(
        &key('j'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd.name(), "cursor-down");
        // Should have the operator count from VimSessionState (4)
        assert_eq!(ctx.count, Some(4));
    } else {
        panic!("expected Execute, got {result:?}");
    }

    // pending_count and pending_register should have been consumed
    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    assert!(vim.pending_count.is_none());
    assert!(vim.pending_register.is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_yy_uses_cursor() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(2, 7);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('y'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::ModeTransition(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(true)));
        // Range should use cursor line (2)
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(2, 0))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(2, 0))
        );
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_yy_with_count_uses_cursor() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(1, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // Set count=3 from VimSessionState
    {
        let vim = extensions.get_or_insert::<crate::VimSessionState>();
        vim.pending_count = Some(3);
    }

    let result = resolver.resolve_with_session(
        &key('y'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::ModeTransition(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(true)));
        // With count=3 and cursor on line 1: start=1, end=3
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(1, 0))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(3, 0))
        );
        assert_eq!(args.get("count"), Some(&reovim_driver_command_types::ArgValue::Count(3)));
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
fn test_resolve_with_session_count_digit() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('7'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );
    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
fn test_resolve_with_session_stores_pending_motion() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("word-forward");
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(1, 3);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    extensions.get_or_insert::<crate::VimSessionState>();

    let result = resolver.resolve_with_session(
        &key('w'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    assert!(matches!(result, ResolveResult::Execute(..)));

    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    assert!(vim.pending_motion.is_some());
    let motion = vim.pending_motion.as_ref().unwrap();
    assert!(!motion.linewise);
    assert!(!motion.inclusive);
    assert!(motion.word_forward);
}

#[test]
fn test_resolve_with_session_linewise_motion() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(0, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    extensions.get_or_insert::<crate::VimSessionState>();

    let result = resolver.resolve_with_session(
        &key('j'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    assert!(matches!(result, ResolveResult::Execute(..)));

    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    let motion = vim.pending_motion.as_ref().unwrap();
    assert!(motion.linewise);
}

#[test]
fn test_resolve_with_session_inclusive_motion() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("line-end");
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(0, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    extensions.get_or_insert::<crate::VimSessionState>();

    let result = resolver.resolve_with_session(
        &key('$'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    assert!(matches!(result, ResolveResult::Execute(..)));

    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    let motion = vim.pending_motion.as_ref().unwrap();
    assert!(!motion.linewise);
    assert!(motion.inclusive);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_cancel_on_not_found() {
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::NotFound,
    };
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('z'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::ModeTransition(ModeTransition::Pop {
        result: Some(PopResult::Cancelled),
    }) = result
    {
        // OK
    } else {
        panic!("expected Cancelled, got {result:?}");
    }
}

// ========================================================================
// on_command_complete tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_with_textobj_range() {
    let resolver = VimYankResolver::with_context(Some(1), None);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let op_state = extensions.get_or_insert::<OperatorPendingState>();
    op_state.set_textobj_range(TextObjRange {
        start: Position::new(0, 2),
        end: Position::new(0, 7),
        is_linewise: false,
    });

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { command, args }),
    }) = result
    {
        assert_eq!(command.name(), "yank");
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 2))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 7))
        );
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(false)));
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_with_linewise_textobj() {
    let resolver = VimYankResolver::with_context(Some(1), Some('a'));
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let op_state = extensions.get_or_insert::<OperatorPendingState>();
    op_state.set_textobj_range(TextObjRange {
        start: Position::new(1, 0),
        end: Position::new(3, 0),
        is_linewise: true,
    });

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(true)));
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(1, 0))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(3, 0))
        );
        if let Some(reovim_driver_command_types::ArgValue::Register(r)) = args.get("register") {
            assert_eq!(*r, 'a');
        } else {
            panic!("expected register 'a'");
        }
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_with_motion_range() {
    let resolver = VimYankResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.initialized = true;
    }

    let mut session = MockSession::with_cursor(0, 5);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { command, args }),
    }) = result
    {
        assert_eq!(command.name(), "yank");
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 0))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 5))
        );
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(false)));
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_inclusive_motion_adjusts_end() {
    let resolver = VimYankResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.initialized = true;
    }

    let mut session = MockSession::with_cursor(0, 4);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, true, false)); // inclusive

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        // End should be adjusted +1 for inclusive motion
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 5))
        );
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_linewise_motion() {
    let resolver = VimYankResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 3));
        state.initialized = true;
    }

    let mut session = MockSession::with_cursor(2, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(true, false, false)); // linewise

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(true)));
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_backward_motion_normalizes_range() {
    let resolver = VimYankResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 10));
        state.initialized = true;
    }

    // Cursor moved backward
    let mut session = MockSession::with_cursor(0, 3);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        // Should normalize: start < end
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 3))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 10))
        );
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
fn test_on_command_complete_no_vim_state_returns_none() {
    let resolver = VimYankResolver::new();
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // No VimSessionState, no OperatorPendingState -> None
    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_none());
}

#[test]
fn test_on_command_complete_no_pending_motion_returns_none() {
    let resolver = VimYankResolver::new();
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    extensions.get_or_insert::<crate::VimSessionState>();

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_none());
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
fn test_resolve_with_keymap_pending_branch() {
    // When keymap returns PrefixOnly, resolve_with_keymap should return Pending
    let resolver = VimYankResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::PrefixOnly,
    };
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending), "PrefixOnly should return Pending");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_init_without_vim_state() {
    // When no VimSessionState is in extensions, the resolver should still work
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(0, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    // Do NOT insert VimSessionState

    let result = resolver.resolve_with_session(
        &key('j'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "cursor-down");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_yy_no_cursor_fallback() {
    // When cursor_position() returns None, yy should fallback to (0,0)-(0,0)
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession {
        mode: VimMode::YANK_ID,
        cursor: None,
    };
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('y'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::ModeTransition(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 0))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 0))
        );
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
fn test_resolve_with_session_pending_branch() {
    // When keymap returns PrefixOnly during resolve_with_session, should return Pending
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::PrefixOnly,
    };
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('g'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    assert!(
        matches!(result, ResolveResult::Pending),
        "PrefixOnly should return Pending, got {result:?}"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_yy_with_motion_count() {
    // Test y3y - motion_count=3 in resolve_with_session
    let resolver = VimYankResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(1, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // First key '3' - motion count
    let result = resolver.resolve_with_session(
        &key('3'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );
    assert!(matches!(result, ResolveResult::Pending));

    // Second key 'y' - line operator
    let result = resolver.resolve_with_session(
        &key('y'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::ModeTransition(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        // count = operator_count(1) * motion_count(3) = 3
        assert_eq!(args.get("count"), Some(&reovim_driver_command_types::ArgValue::Count(3)));
        // Range: start=1, end=3 (3 lines from line 1)
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(1, 0))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(3, 0))
        );
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
fn test_on_command_complete_no_start_position_returns_none() {
    // When start_position is None (motion-based path), should return None
    let resolver = VimYankResolver::new();
    let mut session = MockSession::with_cursor(0, 5);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));
    // Don't set start_position in resolver state

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_none(), "should return None when start_position is not set");
}

#[test]
fn test_on_command_complete_no_cursor_position_returns_none() {
    // When session.cursor_position() returns None, should return None
    let resolver = VimYankResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.initialized = true;
    }

    let mut session = MockSession {
        mode: VimMode::YANK_ID,
        cursor: None,
    };
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_none(), "should return None when cursor_position is None");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_with_register_and_count() {
    let resolver = VimYankResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.operator_count = Some(2);
        state.register = Some('b');
        state.initialized = true;
    }

    let mut session = MockSession::with_cursor(0, 5);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { args, .. }),
    }) = result
    {
        assert_eq!(args.get("count"), Some(&reovim_driver_command_types::ArgValue::Count(2)));
        if let Some(reovim_driver_command_types::ArgValue::Register(r)) = args.get("register") {
            assert_eq!(*r, 'b');
        } else {
            panic!("expected register 'b'");
        }
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

// ========================================================================
// MockSession trait coverage tests
// ========================================================================

#[test]
fn test_mock_session_mode_api() {
    let mut session = MockSession::new();
    assert_eq!(session.current_mode(), &VimMode::YANK_ID);
    assert_eq!(session.home_mode(), &VimMode::YANK_ID);
    assert_eq!(session.mode_depth(), 1);
    assert!(!session.is_mode_active(&VimMode::NORMAL_ID));
    assert_eq!(session.mode_stack(), vec![VimMode::YANK_ID]);
    session.push_mode(VimMode::NORMAL_ID, reovim_driver_session::TransitionContext::default());
    let pop_result = session.pop_mode(None);
    assert!(pop_result.is_ok());
    session.set_mode(VimMode::NORMAL_ID, reovim_driver_session::TransitionContext::default());
    assert_eq!(session.current_mode(), &VimMode::NORMAL_ID);
}
