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
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferId, BufferManager, EventBus, KernelContext, MarkBank,
            MotionEngine, OptionRegistry, Position, RegisterBank, RwLock, SelectionMode,
            TextObjectEngine,
        },
        std::{collections::HashMap, sync::Arc},
    };

    use reovim_module_editor::EDITOR_MODULE;

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
        )
    }

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_enter_visual_command_id() {
        let cmd = EnterVisualMode;
        assert_eq!(cmd.id().name(), "enter-visual");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_enter_visual_line_command_id() {
        let cmd = EnterVisualLineMode;
        assert_eq!(cmd.id().name(), "enter-visual-line");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_enter_visual_block_command_id() {
        let cmd = EnterVisualBlockMode;
        assert_eq!(cmd.id().name(), "enter-visual-block");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_exit_visual_command_id() {
        let cmd = ExitVisualMode;
        assert_eq!(cmd.id().name(), "exit-visual");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    // =========================================================================
    // Entry Command Execution Tests
    // =========================================================================

    #[test]
    fn test_enter_visual_activates_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = EnterVisualMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is active
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_character());
    }

    #[test]
    fn test_enter_visual_line_activates_line_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = EnterVisualLineMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is active and in line mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_line());
    }

    #[test]
    fn test_enter_visual_block_activates_block_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = EnterVisualBlockMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is active and in block mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_block());
    }

    #[test]
    fn test_enter_visual_no_buffer_returns_error() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let args = CommandContext::new();

        let result = EnterVisualMode.execute(&mut ctx, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    // =========================================================================
    // Exit Command Execution Tests
    // =========================================================================

    #[test]
    fn test_exit_visual_clears_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // First enter visual mode
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Verify selection is active
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let buffer = buffer_arc.read();
            assert!(buffer.selection().is_active());
        }

        // Exit visual mode
        let result = ExitVisualMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is cleared
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_exit_visual_without_buffer_succeeds() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let args = CommandContext::new();

        // Should still succeed (just emits event)
        let result = ExitVisualMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // =========================================================================
    // Selection Manipulation Tests
    // =========================================================================

    #[test]
    fn test_swap_anchor_command_id() {
        let cmd = SwapAnchor;
        assert_eq!(cmd.id().name(), "visual-swap-anchor");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_swap_anchor_swaps_positions() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual mode and move cursor
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Move cursor to position 5
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer.set_position(Position::new(0, 5));
        }

        // Verify initial state: anchor at 0, cursor at 5
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let buffer = buffer_arc.read();
            assert_eq!(buffer.selection().anchor, Position::new(0, 0));
            assert_eq!(buffer.position(), Position::new(0, 5));
        }

        // Execute swap anchor
        let result = SwapAnchor.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify swapped: anchor at 5, cursor at 0
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.selection().anchor, Position::new(0, 5));
        assert_eq!(buffer.position(), Position::new(0, 0));
    }

    #[test]
    fn test_swap_anchor_noop_without_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Don't enter visual mode - just execute swap
        let result = SwapAnchor.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_toggle_visual_char_exits_if_already_char() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual mode (character)
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Toggle should exit
        let result = ToggleVisualChar.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_toggle_visual_char_switches_from_line() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual line mode
        let _ = EnterVisualLineMode.execute(&mut ctx, &args);

        // Toggle to char mode
        let result = ToggleVisualChar.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Should be in character mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_character());
    }

    #[test]
    fn test_toggle_visual_line_exits_if_already_line() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual line mode
        let _ = EnterVisualLineMode.execute(&mut ctx, &args);

        // Toggle should exit
        let result = ToggleVisualLine.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_toggle_visual_block_switches_mode() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual mode (character)
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Toggle to block mode
        let result = ToggleVisualBlock.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Should be in block mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_block());
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
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_reselect_last_returns_reselect_visual() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // ReselectLast returns ReselectVisual to signal intent (actual logic is in runner)
        let result = ReselectLast.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::ReselectVisual);
    }

    // =========================================================================
    // Visual Operator Tests
    // =========================================================================

    #[test]
    fn test_delete_selection_command_id() {
        let cmd = DeleteSelection;
        assert_eq!(cmd.id().name(), "delete-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_yank_selection_command_id() {
        let cmd = YankSelection;
        assert_eq!(cmd.id().name(), "yank-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_change_selection_command_id() {
        let cmd = ChangeSelection;
        assert_eq!(cmd.id().name(), "change-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_indent_selection_command_id() {
        let cmd = IndentSelection;
        assert_eq!(cmd.id().name(), "indent-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_dedent_selection_command_id() {
        let cmd = DedentSelection;
        assert_eq!(cmd.id().name(), "dedent-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_delete_selection_noop_without_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = DeleteSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should be unchanged
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello world");
    }

    #[test]
    fn test_yank_selection_noop_without_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = YankSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_change_selection_noop_without_selection() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = ChangeSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_delete_selection_deletes_text() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate selection from position 0 to 5 (selecting "hello")
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Character);
            buffer.set_position(Position::new(0, 4)); // Selecting "hello"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");
    }

    #[test]
    fn test_indent_selection_adds_indentation() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate line selection for lines 0-1
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Line);
            buffer.set_position(Position::new(1, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = IndentSelection.execute(&mut ctx, &args);
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
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("    line1\n    line2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate line selection for lines 0-1
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Line);
            buffer.set_position(Position::new(1, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DedentSelection.execute(&mut ctx, &args);
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
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate character-wise selection for "hello"
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Character);
            buffer.set_position(Position::new(0, 4)); // Selecting "hello"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer content should remain unchanged (yank doesn't delete)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello world");

        // Selection should be cleared after yank
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_change_selection_changes_text() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate character-wise selection for "hello"
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Character);
            buffer.set_position(Position::new(0, 4)); // Selecting "hello"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = ChangeSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted (like delete, but followed by insert mode)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");

        // Selection should be cleared after change
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_delete_selection_line_mode_deletes_entire_lines() {
        use reovim_driver_command::CommandHandler;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("line one\nline two\nline three");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate line-wise selection for lines 0-1
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 3), SelectionMode::Line);
            buffer.set_position(Position::new(1, 2)); // Selection spans lines 0 and 1
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines 0-1 should be completely deleted, leaving only "line three"
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.lines()[0], "line three");
    }
}
