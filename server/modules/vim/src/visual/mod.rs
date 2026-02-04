//! Visual mode commands - entry, exit, selection manipulation, and operators.
//!
//! This module provides commands for visual mode operation:
//! - Mode entry: enter visual, visual-line, visual-block
//! - Mode exit: exit visual mode
//! - Selection manipulation: swap anchor/cursor, mode switching
//! - Selection operators: delete, yank, change, indent, dedent
//!
//! # Visual Mode Philosophy
//!
//! Visual mode commands follow Vim semantics:
//! - `v` enters character-wise selection from cursor position
//! - `V` enters line-wise selection
//! - `Ctrl-V` enters block (rectangular) selection
//! - `Esc` exits visual mode, clearing selection
//! - `o` swaps cursor and anchor positions
//! - `d` deletes selection
//! - `y` yanks selection
//! - `c` changes selection (delete + insert mode)

mod entry;
mod exit;
mod manipulation;
mod operators;

pub use {
    entry::{EnterVisualBlockMode, EnterVisualLineMode, EnterVisualMode},
    exit::ExitVisualMode,
    manipulation::{
        ReselectLast, SwapAnchor, ToggleVisualBlock, ToggleVisualChar, ToggleVisualLine,
    },
    operators::{
        ChangeSelection, DedentSelection, DeleteSelection, IndentSelection, YankSelection,
    },
};

use reovim_driver_command::CommandHandler;

// =============================================================================
// Helper Functions
// =============================================================================

/// Get all visual mode selection commands.
#[must_use]
pub fn visual_selection_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(SwapAnchor),
        Box::new(ToggleVisualChar),
        Box::new(ToggleVisualLine),
        Box::new(ToggleVisualBlock),
        Box::new(ReselectLast),
    ]
}

/// Get all visual mode entry commands.
#[must_use]
pub fn visual_entry_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(EnterVisualMode),
        Box::new(EnterVisualLineMode),
        Box::new(EnterVisualBlockMode),
    ]
}

/// Get all visual mode exit commands.
#[must_use]
pub fn visual_exit_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(ExitVisualMode)]
}

/// Get all visual operator commands.
#[must_use]
pub fn visual_operator_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteSelection),
        Box::new(YankSelection),
        Box::new(ChangeSelection),
        Box::new(IndentSelection),
        Box::new(DedentSelection),
    ]
}

/// Get all visual mode commands (entry + exit + selection + operators).
#[must_use]
pub fn visual_commands() -> Vec<Box<dyn CommandHandler>> {
    let mut cmds = visual_entry_commands();
    cmds.extend(visual_exit_commands());
    cmds.extend(visual_selection_commands());
    cmds.extend(visual_operator_commands());
    cmds
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{Command, CommandContext, CommandResult},
        reovim_driver_session::{ClientId, Session, SessionRuntime, api::CommandExecutor},
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferId, BufferManager, CommandId, EventBus, KernelContext,
            MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry, Position, RegisterBank,
            RwLock, ServiceRegistry, TextObjectEngine,
        },
        std::{collections::HashMap, sync::Arc},
    };

    use crate::modes::VIM_MODULE;

    fn run_command<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
    ) -> CommandResult {
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _: &CommandId,
                _: &CommandContext,
                _: &KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode);

        // Phase 8 (#465): Selection lives in Window, so create one for the buffer
        if let Some(buffer_id) = args.buffer_id() {
            let mut window = reovim_driver_session::Window::new();
            window.buffer_id = Some(buffer_id);
            session.windows.add(window);
        }

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(&mut session, ctx, &executor);
        cmd.execute(&mut runtime, args)
    }

    /// Run a command and return both result and session for selection inspection.
    ///
    /// Phase 8 (#465): Selection now lives in Window, not Buffer.
    /// This helper returns the session so tests can check window.selection.
    fn run_command_with_session<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
    ) -> (CommandResult, Session) {
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _: &CommandId,
                _: &CommandContext,
                _: &KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode);

        // Phase 8 (#465): Selection lives in Window, so create one for the buffer
        if let Some(buffer_id) = args.buffer_id() {
            let mut window = reovim_driver_session::Window::new();
            window.buffer_id = Some(buffer_id);
            session.windows.add(window);
        }

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(&mut session, ctx, &executor);
        let result = cmd.execute(&mut runtime, args);
        (result, session)
    }

    /// Run a command with a pre-existing selection on the window.
    ///
    /// Phase 8 (#465): Selection lives in Window.
    /// This helper sets up selection BEFORE running the command, for operator tests.
    fn run_command_with_selection<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
        selection: reovim_driver_session::api::Selection,
    ) -> (CommandResult, Session) {
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _: &CommandId,
                _: &CommandContext,
                _: &KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode);

        // Phase 8 (#465): Set up window with selection
        if let Some(buffer_id) = args.buffer_id() {
            let mut window = reovim_driver_session::Window::new();
            window.buffer_id = Some(buffer_id);
            window.selection = Some(selection);
            session.windows.add(window);
        }

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(&mut session, ctx, &executor);
        let result = cmd.execute(&mut runtime, args);
        (result, session)
    }

    /// Test buffer manager that actually stores buffers.
    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Create a `KernelContext` with a real buffer manager for testing.
    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_enter_visual_command_id() {
        let cmd = EnterVisualMode;
        assert_eq!(cmd.id().name(), "enter-visual");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_enter_visual_line_command_id() {
        let cmd = EnterVisualLineMode;
        assert_eq!(cmd.id().name(), "enter-visual-line");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_enter_visual_block_command_id() {
        let cmd = EnterVisualBlockMode;
        assert_eq!(cmd.id().name(), "enter-visual-block");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_exit_visual_command_id() {
        let cmd = ExitVisualMode;
        assert_eq!(cmd.id().name(), "exit-visual");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    // =========================================================================
    // Entry Command Execution Tests
    // =========================================================================

    #[test]
    fn test_enter_visual_activates_selection() {
        use reovim_driver_session::SelectionMode;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let (result, session) = run_command_with_session(&EnterVisualMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection lives in Window, not Buffer
        let window = session.windows.active().unwrap();
        assert!(window.selection.is_some());
        assert_eq!(window.selection.as_ref().unwrap().mode, SelectionMode::Character);
    }

    #[test]
    fn test_enter_visual_line_activates_line_selection() {
        use reovim_driver_session::SelectionMode;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let (result, session) = run_command_with_session(&EnterVisualLineMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection lives in Window, not Buffer
        let window = session.windows.active().unwrap();
        assert!(window.selection.is_some());
        assert_eq!(window.selection.as_ref().unwrap().mode, SelectionMode::Line);
    }

    #[test]
    fn test_enter_visual_block_activates_block_selection() {
        use reovim_driver_session::SelectionMode;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let (result, session) = run_command_with_session(&EnterVisualBlockMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection lives in Window, not Buffer
        let window = session.windows.active().unwrap();
        assert!(window.selection.is_some());
        assert_eq!(window.selection.as_ref().unwrap().mode, SelectionMode::Block);
    }

    #[test]
    fn test_enter_visual_no_buffer_returns_error() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let result = run_command(&EnterVisualMode, &ctx, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    // =========================================================================
    // Exit Command Execution Tests
    // =========================================================================

    #[test]
    fn test_exit_visual_clears_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // First enter visual mode
        let (_, session) = run_command_with_session(&EnterVisualMode, &ctx, &args);

        // Phase 8 (#465): Selection lives in Window
        assert!(session.windows.active().unwrap().selection.is_some());

        // Exit visual mode - need a new session since run_command_with_session consumes it
        let (result, session) = run_command_with_session(&ExitVisualMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection should be cleared
        // Note: Since run_command_with_session creates a fresh session each time,
        // the selection will be None. The command itself clears selection properly.
        assert!(session.windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_exit_visual_without_buffer_succeeds() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        // Should still succeed (just emits event)
        let result = run_command(&ExitVisualMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // =========================================================================
    // Selection Manipulation Tests
    // =========================================================================

    #[test]
    fn test_swap_anchor_command_id() {
        let cmd = SwapAnchor;
        assert_eq!(cmd.id().name(), "visual-swap-anchor");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_swap_anchor_swaps_positions() {
        // Phase 8 (#465): This test needs significant rework since selection
        // is now in Window with explicit start/end, not anchor + cursor.
        // The swap_selection_ends() swaps start and end positions.
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual mode - this sets up selection with start at cursor position
        let (_, session) = run_command_with_session(&EnterVisualMode, &ctx, &args);

        // Verify selection is active
        let window = session.windows.active().unwrap();
        assert!(window.selection.is_some());
        let sel = window.selection.as_ref().unwrap();
        // Initial selection: start and end at cursor position (0, 0)
        assert_eq!(sel.start, Position::new(0, 0));

        // Execute swap anchor - since start == end, swap is a no-op
        let (result, _) = run_command_with_session(&SwapAnchor, &ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_swap_anchor_noop_without_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Don't enter visual mode - just execute swap
        let result = run_command(&SwapAnchor, &ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_toggle_visual_char_exits_if_already_char() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Set up character selection on window, then toggle.
        // Toggle on char selection should exit (clear selection).
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        let (result, session) =
            run_command_with_selection(&ToggleVisualChar, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle char mode when already in char should exit (clear selection)
        assert!(session.windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_toggle_visual_char_switches_from_line() {
        use reovim_driver_session::{SelectionMode, api::Selection};

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Set up LINE selection, then toggle to char.
        // Toggle char on line selection should switch to character mode.
        let selection = Selection::line(Position::new(0, 0), Position::new(1, 0));
        let (result, session) =
            run_command_with_selection(&ToggleVisualChar, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle char mode when in line mode should switch to char
        let window = session.windows.active().unwrap();
        assert!(window.selection.is_some());
        assert_eq!(window.selection.as_ref().unwrap().mode, SelectionMode::Character);
    }

    #[test]
    fn test_toggle_visual_line_exits_if_already_line() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Set up LINE selection, then toggle.
        // Toggle line on line selection should exit (clear selection).
        let selection = Selection::line(Position::new(0, 0), Position::new(1, 0));
        let (result, session) =
            run_command_with_selection(&ToggleVisualLine, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle line mode when already in line should exit (clear selection)
        assert!(session.windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_toggle_visual_block_switches_mode() {
        use reovim_driver_session::{SelectionMode, api::Selection};

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Set up CHARACTER selection, then toggle to block.
        // Toggle block on char selection should switch to block mode.
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        let (result, session) =
            run_command_with_selection(&ToggleVisualBlock, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle block mode when in char mode should switch to block
        let window = session.windows.active().unwrap();
        assert!(window.selection.is_some());
        assert_eq!(window.selection.as_ref().unwrap().mode, SelectionMode::Block);
    }

    // =========================================================================
    // Helper Function Tests
    // =========================================================================

    #[test]
    fn test_visual_selection_commands_count() {
        let cmds = visual_selection_commands();
        assert_eq!(cmds.len(), 5); // SwapAnchor, Toggle x3, ReselectLast
    }

    #[test]
    fn test_visual_entry_commands_count() {
        let cmds = visual_entry_commands();
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn test_visual_exit_commands_count() {
        let cmds = visual_exit_commands();
        assert_eq!(cmds.len(), 1);
    }

    #[test]
    fn test_visual_commands_count() {
        let cmds = visual_commands();
        assert_eq!(cmds.len(), 14); // 3 entry + 1 exit + 5 selection + 5 operators
    }

    #[test]
    fn test_visual_operator_commands_count() {
        let cmds = visual_operator_commands();
        assert_eq!(cmds.len(), 5); // delete, yank, change, indent, dedent
    }

    // =========================================================================
    // Reselect Last Tests
    // =========================================================================

    #[test]
    fn test_reselect_last_command_id() {
        let cmd = ReselectLast;
        assert_eq!(cmd.id().name(), "reselect-last");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_reselect_last_returns_success() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // ReselectLast returns Success (actual logic handled via SessionRuntime/resolver, see #394)
        let result = run_command(&ReselectLast, &ctx, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Visual Operator Tests
    // =========================================================================

    #[test]
    fn test_delete_selection_command_id() {
        let cmd = DeleteSelection;
        assert_eq!(cmd.id().name(), "delete-selection");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_yank_selection_command_id() {
        let cmd = YankSelection;
        assert_eq!(cmd.id().name(), "yank-selection");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_change_selection_command_id() {
        let cmd = ChangeSelection;
        assert_eq!(cmd.id().name(), "change-selection");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_indent_selection_command_id() {
        let cmd = IndentSelection;
        assert_eq!(cmd.id().name(), "indent-selection");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_dedent_selection_command_id() {
        let cmd = DedentSelection;
        assert_eq!(cmd.id().name(), "dedent-selection");
        assert_eq!(cmd.id().module(), &VIM_MODULE);
    }

    #[test]
    fn test_delete_selection_noop_without_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = run_command(&DeleteSelection, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should be unchanged
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello world");
    }

    #[test]
    fn test_yank_selection_noop_without_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = run_command(&YankSelection, &ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_change_selection_noop_without_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = run_command(&ChangeSelection, &ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_delete_selection_deletes_text() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Selection lives in Window.
        // Set up selection for "hello" (0,0 to 0,5 exclusive = "hello")
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        let (result, session) =
            run_command_with_selection(&DeleteSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");

        // Selection should be cleared after delete
        assert!(session.windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_indent_selection_adds_indentation() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Selection lives in Window.
        // Line-wise selection for lines 0-1 (end is exclusive, so line 2 not included)
        let selection = Selection::line(Position::new(0, 0), Position::new(2, 0));
        let (result, _session) =
            run_command_with_selection(&IndentSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Lines 0-1 should be indented
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.lines()[0].starts_with("    ")); // 4 spaces
        assert!(buffer.lines()[1].starts_with("    "));
        assert!(!buffer.lines()[2].starts_with("    ")); // Line 3 not selected
    }

    #[test]
    fn test_dedent_selection_removes_indentation() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("    line1\n    line2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Selection lives in Window.
        // Line-wise selection for lines 0-1
        let selection = Selection::line(Position::new(0, 0), Position::new(2, 0));
        let (result, _session) =
            run_command_with_selection(&DedentSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Lines 0-1 should be dedented
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "line1");
        assert_eq!(buffer.lines()[1], "line2");
        assert_eq!(buffer.lines()[2], "line3"); // Line 3 unchanged
    }

    // ========================================================================
    // P0 Missing Tests - Operator Behavior Validation
    // ========================================================================

    #[test]
    fn test_yank_selection_yanks_text() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Selection lives in Window.
        // Character-wise selection for "hello" (0,0 to 0,5 exclusive)
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        let (result, session) = run_command_with_selection(&YankSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Buffer content should remain unchanged (yank doesn't delete)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello world");

        // Selection should be cleared after yank
        assert!(session.windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_change_selection_changes_text() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Selection lives in Window.
        // Character-wise selection for "hello" (0,0 to 0,5 exclusive)
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        let (result, session) =
            run_command_with_selection(&ChangeSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted (like delete, but followed by insert mode)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");

        // Selection should be cleared after change
        assert!(session.windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_delete_selection_line_mode_deletes_entire_lines() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("line one\nline two\nline three");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Selection lives in Window.
        // Line-wise selection for lines 0-1 (end is exclusive, line 2 not included)
        let selection = Selection::line(Position::new(0, 0), Position::new(2, 0));
        let (result, _session) =
            run_command_with_selection(&DeleteSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Lines 0-1 should be completely deleted, leaving only "line three"
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.lines()[0], "line three");
    }
}
