#![allow(clippy::doc_markdown)]

use {
    super::super::visual::*,
    crate::modes::VimMode,
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, KeymapQuery, ModeKeyResolver,
        ModeState, Modifiers, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

fn test_state() -> ModeState {
    ModeState::new(VimMode::VISUAL_ID)
}

struct MockKeymap {
    response: KeyLookupState,
}

impl MockKeymap {
    fn exact_only(cmd: &'static str) -> Self {
        Self {
            response: KeyLookupState::ExactOnly(CommandId::new(ModuleId::new("test"), cmd)),
        }
    }

    fn not_found() -> Self {
        Self {
            response: KeyLookupState::NotFound,
        }
    }

    fn prefix_only() -> Self {
        Self {
            response: KeyLookupState::PrefixOnly,
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
    static MODE: ModeId = VimMode::VISUAL_ID;
    ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
}

// =============================================================================
// Mode Identity Tests
// =============================================================================

#[test]
fn test_mode_id_for_visual() {
    let resolver = VimVisualResolver::character_wise();
    assert_eq!(resolver.mode_id(), &VimMode::VISUAL_ID);
}

#[test]
fn test_mode_id_for_visual_line() {
    let resolver = VimVisualResolver::line_wise();
    assert_eq!(resolver.mode_id(), &VimMode::VISUAL_LINE_ID);
}

#[test]
fn test_mode_id_for_visual_block() {
    let resolver = VimVisualResolver::block_wise();
    assert_eq!(resolver.mode_id(), &VimMode::VISUAL_BLOCK_ID);
}

#[test]
fn test_inherits_from_normal() {
    let resolver = VimVisualResolver::character_wise();
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

// =============================================================================
// Escape Handling Tests
// =============================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_escape_exits_visual_mode() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ctrl_c_exits_visual_mode() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(
        &KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL),
        &mut state,
        &input,
    );

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

#[test]
fn test_escape_clears_pending_state() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    // Build up some state
    resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    assert!(resolver.state().has_motion_count());

    // Escape should clear it
    resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
    assert!(!resolver.state().has_motion_count());
}

// =============================================================================
// Count Accumulation Tests
// =============================================================================

#[test]
fn test_count_digit_accumulates() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(3));

    let result = resolver.resolve_with_keymap(&key('5'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(35));
}

#[test]
fn test_first_zero_not_count_digit() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("line-start");
    let input = resolve_input(&keymap);

    // First 0 is not a count, it's a motion
    let result = resolver.resolve_with_keymap(&key('0'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Execute(..)));
}

#[test]
fn test_subsequent_zero_is_count_digit() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    // Start a count
    resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    // Now 0 is a count digit
    let result = resolver.resolve_with_keymap(&key('0'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Pending));
    assert_eq!(resolver.state().motion_count, Some(30));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_count_flows_to_motion() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);

    // Enter count
    resolver.resolve_with_keymap(&key('3'), &mut state, &input);

    // Execute motion
    let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);
    if let ResolveResult::Execute(_cmd, ctx) = result {
        assert_eq!(ctx.count, Some(3));
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

// =============================================================================
// Motion Delegation Tests
// =============================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_motion_h_executes() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("cursor-left");
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('h'), &mut state, &input);
    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "cursor-left");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_motion_j_executes() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input);
    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "cursor-down");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_motion_w_executes() {
    let resolver = VimVisualResolver::character_wise();
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
fn test_motion_gg_multi_key() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();

    // First g should be pending
    let keymap_prefix = MockKeymap::prefix_only();
    let input_prefix = resolve_input(&keymap_prefix);
    let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input_prefix);
    assert!(matches!(result, ResolveResult::Pending));

    // Second g should execute
    let keymap_exact = MockKeymap::exact_only("document-start");
    let input_exact = resolve_input(&keymap_exact);
    let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input_exact);
    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "document-start");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
fn test_unknown_key_returns_not_handled() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));
}

// =============================================================================
// State Reset Tests
// =============================================================================

#[test]
fn test_reset_clears_state() {
    let mut resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    // Build up state
    resolver.resolve_with_keymap(&key('3'), &mut state, &input);
    assert!(resolver.state().has_motion_count());

    // Reset
    resolver.reset();
    assert!(!resolver.state().has_motion_count());
    assert!(!resolver.state().initialized);
}

// =============================================================================
// Additional Visual Resolver Tests
// =============================================================================

#[test]
fn test_inherits_from_normal_for_all_variants() {
    let line = VimVisualResolver::line_wise();
    assert_eq!(line.inherits_from(), Some(&VimMode::NORMAL_ID));

    let block = VimVisualResolver::block_wise();
    assert_eq!(block.inherits_from(), Some(&VimMode::NORMAL_ID));
}

#[test]
fn test_clear_state_resets_everything() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    // Accumulate state
    resolver.resolve_with_keymap(&key('5'), &mut state, &input);
    assert_eq!(resolver.state().motion_count, Some(5));

    // clear_state via escape
    resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

    let s = resolver.state();
    assert!(s.motion_count.is_none());
    assert!(!s.initialized);
}

#[test]
fn test_multi_digit_count() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    resolver.resolve_with_keymap(&key('1'), &mut state, &input);
    resolver.resolve_with_keymap(&key('2'), &mut state, &input);
    resolver.resolve_with_keymap(&key('3'), &mut state, &input);

    assert_eq!(resolver.state().motion_count, Some(123));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_count_cleared_after_motion_executes() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();

    // Enter count with not-found keymap
    let nf = MockKeymap::not_found();
    let input_nf = resolve_input(&nf);
    resolver.resolve_with_keymap(&key('5'), &mut state, &input_nf);
    assert_eq!(resolver.state().motion_count, Some(5));

    // Execute motion
    let exact = MockKeymap::exact_only("cursor-down");
    let input_exact = resolve_input(&exact);
    let result = resolver.resolve_with_keymap(&key('j'), &mut state, &input_exact);

    if let ResolveResult::Execute(_, ctx) = result {
        assert_eq!(ctx.count, Some(5));
    } else {
        panic!("expected Execute, got {result:?}");
    }

    // Count should be cleared
    assert!(resolver.state().motion_count.is_none());
}

#[test]
fn test_pending_keys_cleared_after_motion_executes() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("cursor-left");
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('h'), &mut state, &input);
    assert!(matches!(result, ResolveResult::Execute(..)));

    // Pending keys should be cleared
    assert!(resolver.state().pending_keys.is_empty());
}

#[test]
fn test_pending_keys_cleared_on_not_found() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    // An unknown key returns NotHandled and clears pending keys
    let result = resolver.resolve_with_keymap(&key('z'), &mut state, &input);
    assert!(matches!(result, ResolveResult::NotHandled));

    // Keys should be cleared
    assert!(resolver.state().pending_keys.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_escape_from_line_wise_mode() {
    let resolver = VimVisualResolver::line_wise();
    let mut state = ModeState::new(VimMode::VISUAL_LINE_ID);
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_ctrl_c_from_block_wise_mode() {
    let resolver = VimVisualResolver::block_wise();
    let mut state = ModeState::new(VimMode::VISUAL_BLOCK_ID);
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(
        &KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL),
        &mut state,
        &input,
    );

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_execute_with_no_count_has_none_in_context() {
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap::exact_only("some-cmd");
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('x'), &mut state, &input);
    if let ResolveResult::Execute(_, ctx) = result {
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
fn test_mode_id_names() {
    let char_wise = VimVisualResolver::character_wise();
    assert_eq!(char_wise.mode_id().name(), "visual");

    let line_wise = VimVisualResolver::line_wise();
    assert_eq!(line_wise.mode_id().name(), "visual-line");

    let block_wise = VimVisualResolver::block_wise();
    assert_eq!(block_wise.mode_id().name(), "visual-block");
}

// =============================================================================
// resolve_with_session tests
// =============================================================================

use {
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{
        WindowError,
        api::{
            BufferApi, ChangeTracker, CommandApi, ModeApi, ModeError, StateChanges, UndoApi,
            WindowApi,
        },
    },
    reovim_kernel::api::v1::{BufferId, UndoResult, WindowId},
    reovim_types_text::{Edit, Position},
};

/// Minimal mock implementing `SessionApiDyn` for resolve_with_session tests.
struct MockSession {
    mode: ModeId,
    cursor: Option<Position>,
}

impl MockSession {
    fn new() -> Self {
        Self {
            mode: VimMode::VISUAL_ID,
            cursor: Some(Position::new(0, 0)),
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
fn test_session_escape_exits_visual_mode() {
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::not_found();
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

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_ctrl_c_exits_visual_mode() {
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_inherits_count_from_normal_mode() {
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // Pre-set pending count in VimSessionState (from normal mode count before entering visual)
    {
        let vim = extensions.get_or_insert::<crate::VimSessionState>();
        vim.pending_count = Some(5);
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
        // Should have inherited count 5 from normal mode
        assert_eq!(ctx.count, Some(5));
    } else {
        panic!("expected Execute, got {result:?}");
    }

    // pending_count should have been consumed
    let vim = extensions.get::<crate::VimSessionState>().unwrap();
    assert!(vim.pending_count.is_none());
}

#[test]
fn test_session_count_digit() {
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('3'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );
    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_motion_executes() {
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("word-forward");
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let result = resolver.resolve_with_session(
        &key('w'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );

    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "word-forward");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
fn test_session_unknown_key_not_handled() {
    let resolver = VimVisualResolver::character_wise();
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

    assert!(matches!(result, ResolveResult::NotHandled));
}

#[test]
fn test_session_pending_key() {
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::prefix_only();
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

    assert!(matches!(result, ResolveResult::Pending));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_no_vim_state_still_works() {
    // Without VimSessionState, initialization should still proceed
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    // No VimSessionState inserted

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
        assert!(ctx.count.is_none());
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_escape_from_line_wise() {
    let resolver = VimVisualResolver::line_wise();
    let mut mstate = ModeState::new(VimMode::VISUAL_LINE_ID);
    let keymap = MockKeymap::not_found();
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

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_escape_from_block_wise() {
    let resolver = VimVisualResolver::block_wise();
    let mut mstate = ModeState::new(VimMode::VISUAL_BLOCK_ID);
    let keymap = MockKeymap::not_found();
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

    if let ResolveResult::Execute(cmd_id, _) = result {
        assert_eq!(cmd_id, crate::ids::EXIT_VISUAL);
    } else {
        panic!("expected Execute(EXIT_VISUAL), got {result:?}");
    }
}

// =========================================================================
// Additional coverage tests
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_keymap_exact_with_longer() {
    // ExactWithLonger is treated as Execute by apply_keymap_policy
    // (in visual mode, ExactWithLonger immediately executes, unlike normal mode)
    let resolver = VimVisualResolver::character_wise();
    let mut state = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::ExactWithLonger {
            exact: CommandId::new(ModuleId::new("test"), "some-cmd"),
        },
    };
    let input = resolve_input(&keymap);

    let result = resolver.resolve_with_keymap(&key('g'), &mut state, &input);
    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "some-cmd");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_resolve_with_session_exact_with_longer() {
    // ExactWithLonger in resolve_with_session also executes immediately
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap {
        response: KeyLookupState::ExactWithLonger {
            exact: CommandId::new(ModuleId::new("test"), "some-cmd"),
        },
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

    if let ResolveResult::Execute(cmd, _) = result {
        assert_eq!(cmd.name(), "some-cmd");
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_session_init_with_vim_state_no_pending_count() {
    // VimSessionState present but pending_count is None
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::exact_only("cursor-down");
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();
    extensions.get_or_insert::<crate::VimSessionState>(); // no pending_count

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
        assert!(ctx.count.is_none());
    } else {
        panic!("expected Execute, got {result:?}");
    }
}

#[test]
fn test_session_count_digit_with_pre_initialized_state() {
    // After initialization, count digits still work on subsequent calls
    let resolver = VimVisualResolver::character_wise();
    let mut mstate = test_state();
    let keymap = MockKeymap::not_found();
    let input = resolve_input(&keymap);
    let mut session = MockSession::new();
    let mut extensions = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    // First call initializes
    let result = resolver.resolve_with_session(
        &key('2'),
        &mut mstate,
        &input,
        &mut session,
        &mut shared_ext,
        &mut extensions,
    );
    assert!(matches!(result, ResolveResult::Pending));

    // Second count digit
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
fn test_new_with_custom_mode() {
    // Test the generic new() constructor
    let custom_mode = ModeId::new(ModuleId::new("vim"), "custom-visual");
    let resolver = VimVisualResolver::new(custom_mode.clone());
    assert_eq!(resolver.mode_id(), &custom_mode);
    assert_eq!(resolver.inherits_from(), Some(&VimMode::NORMAL_ID));
}

// ========================================================================
// MockSession trait coverage tests
// ========================================================================

#[test]
fn test_mock_session_mode_api() {
    let mut session = MockSession::new();
    assert_eq!(session.current_mode(), &VimMode::VISUAL_ID);
    assert_eq!(session.home_mode(), &VimMode::VISUAL_ID);
    assert_eq!(session.mode_depth(), 1);
    assert!(!session.is_mode_active(&VimMode::NORMAL_ID));
    assert_eq!(session.mode_stack(), vec![VimMode::VISUAL_ID]);
    session.push_mode(VimMode::NORMAL_ID, reovim_driver_session::TransitionContext::default());
    let pop_result = session.pop_mode(None);
    assert!(pop_result.is_ok());
    session.set_mode(VimMode::NORMAL_ID, reovim_driver_session::TransitionContext::default());
    assert_eq!(session.current_mode(), &VimMode::NORMAL_ID);
}

#[test]
fn test_mock_session_command_api() {
    let mut session = MockSession::new();
    let cmd_id = CommandId::new(ModuleId::new("test"), "test-cmd");
    let result = session.execute_command(cmd_id, CommandContext::default());
    assert!(matches!(result, CommandResult::Success));
}
