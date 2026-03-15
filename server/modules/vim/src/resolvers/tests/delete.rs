#![allow(clippy::doc_markdown, clippy::equatable_if_let)]

use {
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, KeymapQuery, ModeKeyResolver,
        ModeState, ModeTransition, PopResult, ResolveInput, ResolveResult,
    },
    reovim_driver_session::OperatorPendingState,
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId, Position},
};

use {
    super::super::delete::*,
    crate::{modes::VimMode, session_state::PendingMotion},
};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn test_state() -> ModeState {
    ModeState::new(VimMode::DELETE_ID)
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
    static EMPTY_KEYS: KeySequence = KeySequence::new();
    static MODE: ModeId = VimMode::DELETE_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

#[test]
fn test_new_resolver() {
    let resolver = VimDeleteResolver::new();
    assert_eq!(resolver.mode_id(), &VimMode::DELETE_ID);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_escape_cancels() {
    let resolver = VimDeleteResolver::new();
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
fn test_count_digit() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(3));

    let result = resolver.resolve_with_keymap(&key('5'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(35));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_line_operator_dd() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('d'), &mut state, &input);

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
fn test_motion_key_not_handled() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);
    // With NotFound keymap, motion key results in ModeTransition::Pop with Cancelled
    assert!(matches!(result, ResolveResult::ModeTransition(ModeTransition::Pop { .. })));
}

#[test]
fn test_with_context() {
    let resolver = VimDeleteResolver::with_context(Some(3), Some('a'));
    let state = resolver.state();

    assert_eq!(state.operator_count, Some(3));
    assert_eq!(state.register, Some('a'));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_keymap_executes_motion() {
    let resolver = VimDeleteResolver::new();
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_keymap_cancels_on_not_found() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        assert!(matches!(r, PopResult::Cancelled));
    } else {
        panic!("expected Cancelled, got {result:?}");
    }
}

#[test]
fn test_default_resolver() {
    let resolver = VimDeleteResolver::default();
    assert_eq!(resolver.mode_id(), &VimMode::DELETE_ID);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
fn test_with_context_none_values() {
    let resolver = VimDeleteResolver::with_context(None, None);
    let state = resolver.state();
    assert_eq!(state.operator_count, None);
    assert_eq!(state.register, None);
}

#[test]
fn test_with_context_register() {
    let resolver = VimDeleteResolver::with_context(None, Some('z'));
    let state = resolver.state();
    assert_eq!(state.register, Some('z'));
}

#[test]
fn test_multi_digit_count() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('1'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(1));

    let result = resolver.resolve_with_keymap(&key('0'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(10));
}

#[test]
fn test_reset_clears_state() {
    let mut resolver = VimDeleteResolver::with_context(Some(7), Some('b'));

    // Verify state is set
    let state = resolver.state();
    assert_eq!(state.operator_count, Some(7));

    // Reset
    resolver.reset();

    // State should be cleared
    let state = resolver.state();
    assert_eq!(state.operator_count, None);
    assert_eq!(state.register, None);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_line_operator_dd_with_count() {
    let resolver = VimDeleteResolver::with_context(Some(4), None);
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('d'), &mut state, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_driver_command_types::ArgValue::Bool(true))
            );
            assert_eq!(args.get("count"), Some(&reovim_driver_command_types::ArgValue::Count(4)));
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_line_operator_with_motion_count() {
    // Test d3d - operator count=1 (default), motion count=3
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);

    // Type "3" (motion count)
    let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(3));

    // Type "d" (line operator)
    let result = resolver.resolve_with_keymap(&key('d'), &mut state, &input);

    if let ResolveResult::ModeTransition(ModeTransition::Pop { result: Some(r) }) = result {
        if let PopResult::ExecuteCommand { args, .. } = r {
            assert_eq!(
                args.get("linewise"),
                Some(&reovim_driver_command_types::ArgValue::Bool(true))
            );
            // count = operator_count(1) * motion_count(3) = 3
            assert_eq!(args.get("count"), Some(&reovim_driver_command_types::ArgValue::Count(3)));
        } else {
            panic!("expected ExecuteCommand");
        }
    } else {
        panic!("expected ModeTransition::Pop");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_motion_key_with_count_returns_execute() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("line-end");
    let input = resolve_input(&keymap);

    // Type "2" (count)
    let result = resolver.resolve_with_keymap(&key('2'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));

    // Type "$" (motion)
    let result = resolver.resolve_with_keymap(&key('$'), &mut state, &input);

    if let ResolveResult::Execute(cmd, ctx) = result {
        assert_eq!(cmd.name(), "line-end");
        assert_eq!(ctx.count, Some(2));
    } else {
        panic!("expected Execute, got {result:?}");
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
            mode: VimMode::DELETE_ID,
            cursor: Some(Position::new(0, 0)),
        }
    }

    fn with_cursor(line: usize, col: usize) -> Self {
        Self {
            mode: VimMode::DELETE_ID,
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
    let resolver = VimDeleteResolver::new();
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
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(5, 3);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // Pre-set pending count and register in VimSessionState
    {
        let vim = extensions.get_or_insert::<crate::VimSessionState>();
        vim.pending_count = Some(2);
        vim.pending_register = Some('a');
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
        // Should have the operator count from VimSessionState (2)
        assert_eq!(ctx.count, Some(2));
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
fn test_resolve_with_session_dd_uses_cursor() {
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(3, 5);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('d'),
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
        // Should be linewise
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(true)));
        // Range should use cursor line (3)
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(3, 0))
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
fn test_resolve_with_session_count_digit() {
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('5'),
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
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("word-forward");
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(2, 5);
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

    // Should have stored pending_motion in VimSessionState
    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    assert!(vim.pending_motion.is_some());
    let motion = vim.pending_motion.as_ref().unwrap();
    assert!(!motion.linewise);
    assert!(!motion.inclusive);
    assert!(motion.word_forward); // word-forward is word_forward
}

#[test]
fn test_resolve_with_session_linewise_motion() {
    let resolver = VimDeleteResolver::new();
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
    assert!(vim.pending_motion.is_some());
    let motion = vim.pending_motion.as_ref().unwrap();
    assert!(motion.linewise);
}

#[test]
fn test_resolve_with_session_inclusive_motion() {
    let resolver = VimDeleteResolver::new();
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
    assert!(vim.pending_motion.is_some());
    let motion = vim.pending_motion.as_ref().unwrap();
    assert!(!motion.linewise);
    assert!(motion.inclusive);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_cancel_on_not_found() {
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = MockKeymap::not_found();
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
    let resolver = VimDeleteResolver::with_context(Some(1), None);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // Set up a text object range in OperatorPendingState
    let op_state = extensions.get_or_insert::<OperatorPendingState>();
    op_state.set_textobj_range(TextObjRange {
        start: Position::new(0, 3),
        end: Position::new(0, 8),
        is_linewise: false,
    });

    extensions.get_or_insert::<crate::VimSessionState>();

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { command, args }),
    }) = result
    {
        assert_eq!(command.name(), "delete");
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 3))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 8))
        );
        assert_eq!(args.get("linewise"), Some(&reovim_driver_command_types::ArgValue::Bool(false)));
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }

    // Should have recorded last_change
    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    assert!(vim.last_change.is_some());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_with_motion_range() {
    let resolver = VimDeleteResolver::new();
    // Manually set start position
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.initialized = true;
    }

    let mut session = MockSession::with_cursor(0, 5); // end position after motion
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // Set up pending_motion in VimSessionState (characterwise, not inclusive)
    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some());

    if let Some(ModeTransition::Pop {
        result: Some(PopResult::ExecuteCommand { command, args }),
    }) = result
    {
        assert_eq!(command.name(), "delete");
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
    let resolver = VimDeleteResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.initialized = true;
    }

    let mut session = MockSession::with_cursor(0, 4); // cursor ON last char
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
    let resolver = VimDeleteResolver::new();
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
    let resolver = VimDeleteResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 8));
        state.initialized = true;
    }

    // Cursor moved backward
    let mut session = MockSession::with_cursor(0, 2);
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
            Some(&reovim_driver_command_types::ArgValue::Position(0, 2))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(0, 8))
        );
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
fn test_on_command_complete_no_vim_state_returns_none() {
    let resolver = VimDeleteResolver::new();
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // No VimSessionState, no OperatorPendingState -> None
    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_none());
}

#[test]
fn test_on_command_complete_no_pending_motion_returns_none() {
    let resolver = VimDeleteResolver::new();
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    extensions.get_or_insert::<crate::VimSessionState>();
    // VimSessionState has no pending_motion

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_none());
}

#[test]
fn test_on_command_complete_records_last_change_for_textobj() {
    let resolver = VimDeleteResolver::with_context(Some(2), Some('a'));
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let op_state = extensions.get_or_insert::<OperatorPendingState>();
    op_state.set_textobj_range(TextObjRange {
        start: Position::new(1, 0),
        end: Position::new(1, 5),
        is_linewise: true,
    });

    extensions.get_or_insert::<crate::VimSessionState>();

    let _ = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);

    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    let lc = vim.last_change.as_ref().unwrap();
    assert!(matches!(
        lc.change_type,
        crate::session_state::ChangeType::OperatorTextObject { .. }
    ));
    assert_eq!(lc.count, Some(2));
    assert_eq!(lc.register, Some('a'));
}

// ========================================================================
// pending_keys() tests (#468 which-key for operator modes)
// ========================================================================

#[test]
fn test_pending_keys_empty_initially() {
    let resolver = VimDeleteResolver::new();
    let keys = resolver.pending_keys();
    assert!(keys.is_empty(), "fresh resolver should have no pending keys");
}

#[test]
fn test_pending_keys_returns_accumulated_keys() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::PrefixOnly,
    };
    let input = resolve_input(&keymap);

    // Type 'g' - keymap returns PrefixOnly → Pending
    let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));

    let keys = resolver.pending_keys();
    assert_eq!(keys.len(), 1, "should have 1 pending key after pressing 'g'");
}

#[test]
fn test_pending_keys_cleared_after_cancel() {
    let resolver = VimDeleteResolver::new();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::PrefixOnly,
    };
    let input = resolve_input(&keymap);

    // Accumulate a pending key
    let _ = resolver.resolve_with_keymap(&key('g'), &mut state, &input);
    assert!(!resolver.pending_keys().is_empty());

    // Escape clears state
    let _ = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
    assert!(
        resolver.pending_keys().is_empty(),
        "pending keys should be cleared after escape"
    );
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
fn test_resolve_with_keymap_pending_branch() {
    // When keymap returns PrefixOnly, resolve_with_keymap should return Pending
    let resolver = VimDeleteResolver::new();
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
    let resolver = VimDeleteResolver::new();
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
fn test_resolve_with_session_dd_no_cursor_fallback() {
    // When cursor_position() returns None, dd should fallback to (0,0)-(0,0)
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession {
        mode: VimMode::DELETE_ID,
        cursor: None,
    };
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('d'),
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
    let resolver = VimDeleteResolver::new();
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
fn test_resolve_with_session_dd_with_count_uses_cursor() {
    // dd with count from VimSessionState should use cursor line
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(2, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    {
        let vim = extensions.get_or_insert::<crate::VimSessionState>();
        vim.pending_count = Some(3);
    }

    let result = resolver.resolve_with_session(
        &key('d'),
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
        // start=2, end=4 (3 lines from line 2)
        assert_eq!(
            args.get("range_start"),
            Some(&reovim_driver_command_types::ArgValue::Position(2, 0))
        );
        assert_eq!(
            args.get("range_end"),
            Some(&reovim_driver_command_types::ArgValue::Position(4, 0))
        );
        assert_eq!(args.get("count"), Some(&reovim_driver_command_types::ArgValue::Count(3)));
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_dd_with_motion_count() {
    // Test d3d - motion_count=3 in resolve_with_session
    let resolver = VimDeleteResolver::new();
    let mut mstate = test_state();
    let keymap = NotFoundKeymap;
    let input = resolve_input(&keymap);
    let mut session = MockSession::with_cursor(0, 0);
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

    // Second key 'd' - line operator
    let result = resolver.resolve_with_session(
        &key('d'),
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
    } else {
        panic!("expected Pop with ExecuteCommand, got {result:?}");
    }
}

#[test]
fn test_on_command_complete_no_start_position_returns_none() {
    // When start_position is None (motion-based path), should return None
    let resolver = VimDeleteResolver::new();
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
    let resolver = VimDeleteResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.initialized = true;
    }

    let mut session = MockSession {
        mode: VimMode::DELETE_ID,
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
fn test_on_command_complete_records_last_change_for_motion() {
    let resolver = VimDeleteResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(0, 0));
        state.register = Some('b');
        state.operator_count = Some(3);
        state.initialized = true;
    }

    let mut session = MockSession::with_cursor(0, 5);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let _ = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);

    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    let lc = vim.last_change.as_ref().unwrap();
    assert!(matches!(
        lc.change_type,
        crate::session_state::ChangeType::OperatorMotion { .. }
    ));
    assert_eq!(lc.count, Some(3));
    assert_eq!(lc.register, Some('b'));
}

// ========================================================================
// MockSession trait coverage tests
// ========================================================================

#[test]
fn test_mock_session_mode_api() {
    let mut session = MockSession::new();
    assert_eq!(session.current_mode(), &VimMode::DELETE_ID);
    assert_eq!(session.home_mode(), &VimMode::DELETE_ID);
    assert_eq!(session.mode_depth(), 1);
    assert!(!session.is_mode_active(&VimMode::NORMAL_ID));
    assert_eq!(session.mode_stack(), vec![VimMode::DELETE_ID]);
    session.push_mode(VimMode::NORMAL_ID, reovim_driver_session::TransitionContext::default());
    let pop_result = session.pop_mode(None);
    assert!(pop_result.is_ok());
    session.set_mode(VimMode::NORMAL_ID, reovim_driver_session::TransitionContext::default());
    assert_eq!(session.current_mode(), &VimMode::NORMAL_ID);
}

#[test]
fn test_mock_session_buffer_api() {
    let mut session = MockSession::new();
    let bid = BufferId::new();
    assert!(session.active_buffer().is_none());
    assert!(session.buffer_line(bid, 0).is_none());
    assert!(session.buffer_line_count(bid).is_none());
    assert!(session.buffer_line_len(bid, 0).is_none());
    assert!(
        session
            .buffer_text_range(bid, Position::new(0, 0), Position::new(0, 1))
            .is_none()
    );
    assert!(session.buffer_content(bid).is_none());
    assert!(session.buffer_file_path(bid).is_none());
    assert!(session.is_buffer_modified(bid).is_none());
    session.set_buffer_modified(bid, true);
    session.insert_text(bid, Position::new(0, 0), "text");
    session.delete_range(bid, Position::new(0, 0), Position::new(0, 1));
    let _ = session.create_buffer(None, "content");
    let _ = session.delete_buffer(bid);
    session.rename_buffer(bid, "new-name");
}

#[test]
fn test_mock_session_window_api() {
    let mut session = MockSession::new();
    let wid = session.active_window().unwrap();
    assert_eq!(session.cursor_position(), Some(Position::new(0, 0)));
    assert_eq!(session.window_count(), 1);
    assert!(session.window_buffer(wid).is_none());
    let _ = session.create_window(None);
    let _ = session.close_window(wid);
    let _ = session.focus_window(wid);
    let _ = session.set_window_buffer(wid, BufferId::new());
}

#[test]
fn test_mock_session_command_api() {
    let mut session = MockSession::new();
    let cmd_id = CommandId::new(TEST_MODULE, "test-cmd");
    let result = session.execute_command(cmd_id, CommandContext::default());
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_mock_session_undo_api() {
    let mut session = MockSession::new();
    let bid = BufferId::new();
    assert!(session.undo(bid).is_none());
    assert!(session.redo(bid).is_none());
    session.record_edit(bid, vec![], Position::new(0, 0), Position::new(0, 0));
    assert!(!session.can_undo(bid));
    assert!(!session.can_redo(bid));
    assert!(session.undo_mine(bid).is_none());
    assert!(session.redo_mine(bid).is_none());
    session.record_edit_mine(bid, vec![], Position::new(0, 0), Position::new(0, 0));
}

// ========================================================================
// Deferred motion tests (#663 operator-pending jump integration)
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_defers_when_cursor_unmoved() {
    // When a multi-step motion (e.g., jump search) pushes a mode for label
    // selection, on_command_complete fires before the cursor has moved.
    // The resolver should return None and NOT consume pending_motion.
    let resolver = VimDeleteResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(2, 5));
        state.initialized = true;
    }

    // Cursor is at SAME position as start (motion hasn't completed yet)
    let mut session = MockSession::with_cursor(2, 5);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_none(), "should defer when cursor hasn't moved");

    // pending_motion must NOT be consumed (preserved for second call)
    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    assert!(
        vim.pending_motion.is_some(),
        "pending_motion should be preserved for deferred motion"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_on_command_complete_completes_after_cursor_moves() {
    // Second call: cursor has moved to the jump target.
    let resolver = VimDeleteResolver::new();
    {
        let mut state = resolver.state.write().unwrap();
        state.set_start_position(Position::new(2, 5));
        state.initialized = true;
    }

    // Cursor moved to (4, 0) — different from start (2, 5)
    let mut session = MockSession::with_cursor(4, 0);
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let vim = extensions.get_or_insert::<crate::VimSessionState>();
    vim.pending_motion = Some(PendingMotion::new(false, false, false));

    let result = resolver.on_command_complete(&mut session, &mut shared_ext, &mut extensions);
    assert!(result.is_some(), "should complete when cursor moved");

    // pending_motion should now be consumed
    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    assert!(vim.pending_motion.is_none(), "pending_motion should be consumed");
}

#[test]
fn test_mock_session_change_tracker() {
    let mut session = MockSession::new();
    let changes = session.take_changes();
    assert!(!changes.has_changes());
    session.record_cursor_move(BufferId::new());
}
