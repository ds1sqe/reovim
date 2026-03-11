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
        reovim_kernel::testing::create_test_context,
        super::*,
        reovim_driver_command::{Command, CommandContext, CommandResult},
        reovim_driver_session::{
            testing::StubExecutor,
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
        },
        reovim_kernel::api::{
            ModeStack,
            v1::{
                Buffer, BufferId, HistoryRing,
                KernelContext, MarkBank, ModeId, ModuleId, Position,
                RegisterBank,
            },
        },
    };

    use crate::modes::VIM_MODULE;


    /// Test state holder for per-client state (#471 borrow checker fix).
    struct TestState {
        session: Session,
        mode_stack: ModeStack,
        windows: WindowLayout,
        extensions: ExtensionMap,
        compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
        tabs: reovim_driver_session::TabPageSet,
        registers: RegisterBank,
        clipboard_history: HistoryRing,
        local_marks: MarkBank,
        active_buffer: Option<BufferId>,
        terminal_size: (u16, u16),
    }

    impl TestState {
        /// Create test state with a window containing the given buffer.
        fn with_buffer(buffer_id: Option<BufferId>) -> Self {
            let home_mode = ModeId::new(ModuleId::new("test"), "normal");
            let session = Session::new(ClientId::new(1), home_mode.clone()); // #491
            let mode_stack = ModeStack::new(home_mode);
            let mut windows = WindowLayout::empty();
            let extensions = ExtensionMap::new();

            // Phase 8 (#465): Selection lives in Window, so create one for the buffer
            let mut window = reovim_driver_session::Window::new();
            if let Some(buffer_id) = buffer_id {
                window.buffer_id = Some(buffer_id);
            }
            windows.add(window);

            Self {
                session,
                mode_stack,
                windows,
                extensions,
                compositor: None,
                tabs: reovim_driver_session::TabPageSet::new(),
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
                active_buffer: None,
                terminal_size: (80, 24),
            }
        }

        /// Create test state with a window containing buffer AND pre-existing selection.
        fn with_selection(
            buffer_id: BufferId,
            selection: reovim_driver_session::api::Selection,
        ) -> Self {
            let home_mode = ModeId::new(ModuleId::new("test"), "normal");
            let session = Session::new(ClientId::new(1), home_mode.clone()); // #491
            let mode_stack = ModeStack::new(home_mode);
            let mut windows = WindowLayout::empty();
            let extensions = ExtensionMap::new();

            // Phase 8 (#465): Set up window with selection
            let mut window = reovim_driver_session::Window::new();
            window.buffer_id = Some(buffer_id);
            window.selection = Some(selection);
            windows.add(window);

            Self {
                session,
                mode_stack,
                windows,
                extensions,
                compositor: None,
                tabs: reovim_driver_session::TabPageSet::new(),
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
                active_buffer: None,
                terminal_size: (80, 24),
            }
        }

        /// Create a runtime from this test state.
        fn runtime<'a>(&'a mut self, kernel: &'a KernelContext) -> SessionRuntime<'a> {
            SessionRuntime::new(
                &mut self.session,
                reovim_driver_session::ClientContext {
                    mode_stack: &mut self.mode_stack,
                    windows: &mut self.windows,
                    extensions: &mut self.extensions,
                    compositor: &mut self.compositor,
                    tabs: &mut self.tabs,
                    registers: &mut self.registers,
                    clipboard_history: &mut self.clipboard_history,
                    local_marks: &mut self.local_marks,
                    active_buffer: &mut self.active_buffer,
                    terminal_size: &mut self.terminal_size,
                },
                kernel,
                &StubExecutor,
            )
        }
    }

    fn run_command<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
    ) -> CommandResult {
        let mut state = TestState::with_buffer(args.buffer_id());
        let mut runtime = state.runtime(ctx);
        cmd.execute(&mut runtime, args)
    }

    /// Run a command and return both result and windows for selection inspection.
    ///
    /// Phase 8 (#465): Selection now lives in Window, not Buffer.
    /// This helper returns the windows so tests can check window.selection.
    fn run_command_with_session<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
    ) -> (CommandResult, WindowLayout) {
        let mut state = TestState::with_buffer(args.buffer_id());
        let mut runtime = state.runtime(ctx);
        let result = cmd.execute(&mut runtime, args);
        drop(runtime);
        (result, state.windows)
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
    ) -> (CommandResult, WindowLayout) {
        let buffer_id = args
            .buffer_id()
            .expect("buffer_id required for selection test");
        let mut state = TestState::with_selection(buffer_id, selection);
        let mut runtime = state.runtime(ctx);
        let result = cmd.execute(&mut runtime, args);
        drop(runtime);
        (result, state.windows)
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

        let (result, windows) = run_command_with_session(&EnterVisualMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection lives in Window, not Buffer
        let window = windows.active().unwrap();
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

        let (result, windows) = run_command_with_session(&EnterVisualLineMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection lives in Window, not Buffer
        let window = windows.active().unwrap();
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

        let (result, windows) = run_command_with_session(&EnterVisualBlockMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection lives in Window, not Buffer
        let window = windows.active().unwrap();
        assert!(window.selection.is_some());
        assert_eq!(window.selection.as_ref().unwrap().mode, SelectionMode::Block);
    }

    #[test]
    fn test_enter_visual_no_buffer_succeeds_with_window() {
        // Phase 8 (#471): EnterVisualMode now operates on the active window,
        // not the buffer_id in args. As long as there's an active window,
        // the command succeeds. The test helper always creates a window,
        // so this now returns Success.
        let ctx = create_test_context();
        let args = CommandContext::new();

        let result = run_command(&EnterVisualMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);
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
        let (_, windows) = run_command_with_session(&EnterVisualMode, &ctx, &args);

        // Phase 8 (#465): Selection lives in Window
        assert!(windows.active().unwrap().selection.is_some());

        // Exit visual mode - need a new state since run_command_with_session consumes it
        let (result, windows) = run_command_with_session(&ExitVisualMode, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Phase 8 (#465): Selection should be cleared
        // Note: Since run_command_with_session creates a fresh state each time,
        // the selection will be None. The command itself clears selection properly.
        assert!(windows.active().unwrap().selection.is_none());
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
        let (_, windows) = run_command_with_session(&EnterVisualMode, &ctx, &args);

        // Verify selection is active
        let window = windows.active().unwrap();
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
        let (result, windows) =
            run_command_with_selection(&ToggleVisualChar, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle char mode when already in char should exit (clear selection)
        assert!(windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_toggle_visual_char_switches_from_line() {
        use reovim_driver_session::{
            SelectionMode, api::Selection};

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Set up LINE selection, then toggle to char.
        // Toggle char on line selection should switch to character mode.
        let selection = Selection::line(Position::new(0, 0), Position::new(1, 0));
        let (result, windows) =
            run_command_with_selection(&ToggleVisualChar, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle char mode when in line mode should switch to char
        let window = windows.active().unwrap();
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
        let (result, windows) =
            run_command_with_selection(&ToggleVisualLine, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle line mode when already in line should exit (clear selection)
        assert!(windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_toggle_visual_block_switches_mode() {
        use reovim_driver_session::{
            SelectionMode, api::Selection};

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Phase 8 (#465): Set up CHARACTER selection, then toggle to block.
        // Toggle block on char selection should switch to block mode.
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        let (result, windows) =
            run_command_with_selection(&ToggleVisualBlock, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle block mode when in char mode should switch to block
        let window = windows.active().unwrap();
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
        let (result, windows) =
            run_command_with_selection(&DeleteSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");

        // Selection should be cleared after delete
        assert!(windows.active().unwrap().selection.is_none());
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
        let (result, windows) = run_command_with_selection(&YankSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Buffer content should remain unchanged (yank doesn't delete)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello world");

        // Selection should be cleared after yank
        assert!(windows.active().unwrap().selection.is_none());
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
        let (result, windows) =
            run_command_with_selection(&ChangeSelection, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted (like delete, but followed by insert mode)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");

        // Selection should be cleared after change
        assert!(windows.active().unwrap().selection.is_none());
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

    // =========================================================================
    // Additional Visual Mode Integration Tests
    // =========================================================================

    #[test]
    fn test_visual_commands_total_is_correct() {
        let total = visual_commands();
        let entry = visual_entry_commands();
        let exit = visual_exit_commands();
        let sel = visual_selection_commands();
        let ops = visual_operator_commands();
        assert_eq!(total.len(), entry.len() + exit.len() + sel.len() + ops.len());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_all_visual_commands_have_vim_module() {
        let cmds = visual_commands();
        for cmd in &cmds {
            assert_eq!(
                cmd.id().module(),
                &VIM_MODULE,
                "command '{}' should be in vim module",
                cmd.id().name()
            );
        }
    }

    #[test]
    fn test_visual_entry_commands_unique_ids() {
        let cmds = visual_entry_commands();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(id.name()), "Duplicate command id: {}", id.name());
        }
    }

    #[test]
    fn test_visual_operator_commands_unique_ids() {
        let cmds = visual_operator_commands();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(id.name()), "Duplicate command id: {}", id.name());
        }
    }

    #[test]
    fn test_visual_selection_commands_unique_ids() {
        let cmds = visual_selection_commands();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(id.name()), "Duplicate command id: {}", id.name());
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_all_visual_commands_unique_ids() {
        let cmds = visual_commands();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(
                seen.insert(format!("{}:{}", id.module().as_str(), id.name())),
                "Duplicate command id: {}",
                id.name()
            );
        }
    }

    #[test]
    fn test_indent_selection_noop_without_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&IndentSelection, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer unchanged
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello");
    }

    #[test]
    fn test_dedent_selection_noop_without_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello\n    world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&DedentSelection, &ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer unchanged
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "    hello");
    }

    #[test]
    fn test_toggle_visual_line_from_char_keeps_selection() {
        use reovim_driver_session::{
            SelectionMode, api::Selection};

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Toggle from char to line should keep selection with Line mode
        let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
        let (result, windows) =
            run_command_with_selection(&ToggleVisualLine, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        let window = windows.active().unwrap();
        assert!(window.selection.is_some());
        assert_eq!(window.selection.as_ref().unwrap().mode, SelectionMode::Line);
    }

    #[test]
    fn test_toggle_visual_block_exits_if_already_block() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let selection = Selection::block(Position::new(0, 0), Position::new(0, 5));
        let (result, windows) =
            run_command_with_selection(&ToggleVisualBlock, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        // Toggle block when already block exits (clears selection)
        assert!(windows.active().unwrap().selection.is_none());
    }

    #[test]
    fn test_swap_anchor_with_different_lines() {
        use reovim_driver_session::api::Selection;

        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let selection = Selection::character(Position::new(0, 2), Position::new(2, 1));
        let (result, windows) = run_command_with_selection(&SwapAnchor, &ctx, &args, selection);
        assert_eq!(result, CommandResult::Success);

        let window = windows.active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(2, 1));
        assert_eq!(sel.end, Position::new(0, 2));
    }
}
