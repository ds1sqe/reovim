//! Editor commands - cursor movement, mode switching, and text operations.
//!
//! This module provides the basic commands for editor operation:
//! - Cursor movement: up, down, left, right
//! - Display line movement: gj, gk
//! - Mode switching: enter insert, exit to normal, window mode
//! - Mode entry: I, A, o, O
//! - Insert mode edits: newline, tab
//! - Delete operations: x, X, dd, D
//! - Change operations: cc, C
//! - Replace operations: r, .
//! - Undo/redo: u, Ctrl-R
//! - Yank: yy, Y
//! - Paste: p, P
//! - File operations: :w
//!
//! # Cursor Movement Philosophy
//!
//! Cursor movement commands follow Vim semantics:
//! - j/k (down/up) preserve the "preferred column" - the column the user
//!   intended, even if shorter lines force temporary repositioning
//! - h/l (left/right) clear the preferred column
//! - Movements clamp to valid positions (no-op at boundaries)

mod cursor;
mod delete;
mod display_line;
mod file;
mod insert_edit;
mod mode;
mod mode_entry;
mod paste;
mod replace;
mod undo;
mod yank;

// Re-export all command types for external use
pub use {
    cursor::{CursorDown, CursorLeft, CursorRight, CursorUp},
    delete::{
        ChangeLine, ChangeToEndOfLine, DeleteChar, DeleteCharBefore, DeleteLine, DeleteToEndOfLine,
    },
    display_line::{CursorDisplayDown, CursorDisplayUp},
    file::WriteBufferCommand,
    insert_edit::{InsertNewline, InsertTab},
    mode::{EnterInsertMode, EnterInsertModeAppend, EnterWindowMode, ExitToNormal},
    mode_entry::{EnterInsertEndOfLine, EnterInsertFirstNonBlank, OpenLineAbove, OpenLineBelow},
    paste::{PasteAfter, PasteBefore},
    replace::{JoinLines, RepeatDot, ReplaceCharStart},
    undo::{RedoCommand, UndoCommand},
    yank::YankLine,
};

use reovim_driver_command::CommandHandler;

// =============================================================================
// Command Registration Helper
// =============================================================================

/// Get all editor commands as boxed trait objects.
///
/// This is useful for registering all commands at once.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        // Cursor movement
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
        // Display line movement
        Box::new(CursorDisplayDown),
        Box::new(CursorDisplayUp),
        // Mode switching
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(EnterInsertFirstNonBlank),
        Box::new(EnterInsertEndOfLine),
        Box::new(OpenLineBelow),
        Box::new(OpenLineAbove),
        Box::new(ExitToNormal),
        Box::new(EnterWindowMode),
        // Insert mode edits
        Box::new(InsertNewline),
        Box::new(InsertTab),
        // Delete operations
        Box::new(DeleteChar),
        Box::new(DeleteCharBefore),
        Box::new(DeleteLine),
        Box::new(DeleteToEndOfLine),
        Box::new(JoinLines),
        // Yank
        Box::new(YankLine),
        // Paste
        Box::new(PasteAfter),
        Box::new(PasteBefore),
        // Change
        Box::new(ChangeLine),
        Box::new(ChangeToEndOfLine),
        // Replace
        Box::new(ReplaceCharStart),
        // Repeat
        Box::new(RepeatDot),
        // Undo/redo
        Box::new(UndoCommand),
        Box::new(RedoCommand),
        // File operations
        Box::new(WriteBufferCommand),
    ]
}

/// Get all cursor movement commands.
#[must_use]
pub fn cursor_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
    ]
}

/// Get display line movement commands (gj, gk).
#[must_use]
pub fn display_line_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(CursorDisplayDown), Box::new(CursorDisplayUp)]
}

/// Get all mode switching commands.
#[must_use]
pub fn mode_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(EnterInsertFirstNonBlank),
        Box::new(EnterInsertEndOfLine),
        Box::new(OpenLineBelow),
        Box::new(OpenLineAbove),
        Box::new(ExitToNormal),
        Box::new(EnterWindowMode),
    ]
}

/// Get all insert mode edit commands.
#[must_use]
pub fn insert_edit_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(InsertNewline), Box::new(InsertTab)]
}

/// Get all delete commands.
#[must_use]
pub fn delete_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteChar),
        Box::new(DeleteCharBefore),
        Box::new(DeleteLine),
        Box::new(DeleteToEndOfLine),
        Box::new(JoinLines),
    ]
}

/// Get all undo/redo commands.
#[must_use]
pub fn undo_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(UndoCommand), Box::new(RedoCommand)]
}

/// Get all yank commands.
#[must_use]
pub fn yank_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(YankLine)]
}

/// Get all paste commands.
#[must_use]
pub fn paste_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(PasteAfter), Box::new(PasteBefore)]
}

/// Get all change commands.
#[must_use]
pub fn change_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(ChangeLine), Box::new(ChangeToEndOfLine)]
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferId, BufferManager, EventBus, KernelContext, MarkBank,
            MotionEngine, OptionRegistry, Position, RegisterBank, RegisterContent, RwLock,
            TextObjectEngine,
        },
        std::{collections::HashMap, sync::Arc},
    };

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
    fn test_cursor_up_id() {
        let cmd = CursorUp;
        assert_eq!(cmd.id().name(), "cursor-up");
    }

    #[test]
    fn test_cursor_down_id() {
        let cmd = CursorDown;
        assert_eq!(cmd.id().name(), "cursor-down");
    }

    #[test]
    fn test_cursor_left_id() {
        let cmd = CursorLeft;
        assert_eq!(cmd.id().name(), "cursor-left");
    }

    #[test]
    fn test_cursor_right_id() {
        let cmd = CursorRight;
        assert_eq!(cmd.id().name(), "cursor-right");
    }

    #[test]
    fn test_enter_insert_id() {
        let cmd = EnterInsertMode;
        assert_eq!(cmd.id().name(), "enter-insert");
    }

    #[test]
    fn test_enter_insert_append_id() {
        let cmd = EnterInsertModeAppend;
        assert_eq!(cmd.id().name(), "enter-insert-after");
    }

    #[test]
    fn test_exit_to_normal_id() {
        let cmd = ExitToNormal;
        assert_eq!(cmd.id().name(), "exit-insert");
    }

    // =========================================================================
    // Command Args Tests
    // =========================================================================

    #[test]
    fn test_cursor_commands_have_count_arg() {
        for cmd in cursor_commands() {
            let args = cmd.args();
            assert!(!args.is_empty(), "Command {} should have count arg", cmd.id());
            assert_eq!(args[0].name, "count");
            assert_eq!(args[0].kind, ArgKind::Count);
        }
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        // 4 cursor + 2 display + 8 mode + 2 insert-edit + 5 delete + 1 yank + 2 paste
        // + 2 change + 2 undo + 1 replace_char + 1 repeat + 1 write = 31
        assert_eq!(cmds.len(), 31);
    }

    #[test]
    fn test_cursor_commands_count() {
        let cmds = cursor_commands();
        assert_eq!(cmds.len(), 4);
    }

    #[test]
    fn test_display_line_commands_count() {
        let cmds = display_line_commands();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_mode_commands_count() {
        let cmds = mode_commands();
        assert_eq!(cmds.len(), 8);
    }

    // =========================================================================
    // Cursor Commands Without Buffer ID (Error Handling)
    // =========================================================================

    #[test]
    fn test_cursor_up_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_down_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_left_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorLeft.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_right_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Cursor Commands With Invalid Buffer ID (Error Handling)
    // =========================================================================

    #[test]
    fn test_cursor_up_invalid_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(999));
        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Cursor Movement Tests (With Valid Buffer)
    // =========================================================================

    fn setup_buffer_context() -> (KernelContext, BufferId) {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line one\nline two\nline three");
        let buffer_id = ctx.buffers.register(buffer);
        (ctx, buffer_id)
    }

    #[test]
    fn test_cursor_down_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_cursor_up_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // First move down, then test up
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(2, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1);
    }

    #[test]
    fn test_cursor_right_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 1);
    }

    #[test]
    fn test_cursor_left_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // First move right, then test left
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 5));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorLeft.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 4);
    }

    // =========================================================================
    // Count Argument Tests
    // =========================================================================

    #[test]
    fn test_cursor_down_count_respected() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 2); // Moved down 2 lines
    }

    #[test]
    fn test_cursor_right_count_respected() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 3);
    }

    // =========================================================================
    // Boundary Tests
    // =========================================================================

    #[test]
    fn test_cursor_up_at_line_zero_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Cursor starts at (0, 0), should stay there
        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 0);
    }

    #[test]
    fn test_cursor_down_at_eof_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // Move to last line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(2, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 2); // Stayed at last line
    }

    #[test]
    fn test_cursor_left_at_col_zero_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Cursor starts at column 0
        let result = CursorLeft.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_cursor_right_at_eol_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // Move to last character of line ("line one" is 8 chars, last char at index 7)
        // In normal mode, cursor can't go past the last character
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 7)); // 'e' in "line one"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 7); // Stayed at last character
    }

    // =========================================================================
    // Mode Commands Tests
    // =========================================================================

    #[test]
    fn test_mode_commands_execute_success() {
        let mut ctx = create_test_context();
        let args = CommandContext::new();

        // Mode commands don't require buffer_id
        assert_eq!(
            EnterInsertMode.execute(&mut ctx, &args),
            reovim_driver_command::CommandResult::Success
        );
        assert_eq!(
            EnterInsertModeAppend.execute(&mut ctx, &args),
            reovim_driver_command::CommandResult::Success
        );
        assert_eq!(
            ExitToNormal.execute(&mut ctx, &args),
            reovim_driver_command::CommandResult::Success
        );
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_cursor_movement_empty_buffer() {
        let mut ctx = create_test_context();
        let buffer = Buffer::new(); // Empty buffer
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // All movements should succeed (no-op on empty buffer)
        assert!(CursorDown.execute(&mut ctx, &args).is_success());
        assert!(CursorUp.execute(&mut ctx, &args).is_success());
        assert!(CursorLeft.execute(&mut ctx, &args).is_success());
        assert!(CursorRight.execute(&mut ctx, &args).is_success());
    }

    #[test]
    fn test_cursor_movement_single_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("single line");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Up/down should be no-op
        assert!(CursorDown.execute(&mut ctx, &args).is_success());
        assert!(CursorUp.execute(&mut ctx, &args).is_success());

        // Left/right should work
        assert!(CursorRight.execute(&mut ctx, &args).is_success());
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().position().column, 1);
    }

    // =========================================================================
    // Undo/Redo Command Tests
    // =========================================================================

    #[test]
    fn test_undo_command_id() {
        let cmd = UndoCommand;
        assert_eq!(cmd.id().name(), "undo");
    }

    #[test]
    fn test_redo_command_id() {
        let cmd = RedoCommand;
        assert_eq!(cmd.id().name(), "redo");
    }

    #[test]
    fn test_undo_command_names() {
        let cmd = UndoCommand;
        let names = cmd.names();
        assert!(names.contains(&"u"));
        assert!(names.contains(&"undo"));
    }

    #[test]
    fn test_redo_command_names() {
        let cmd = RedoCommand;
        let names = cmd.names();
        assert!(names.contains(&"redo"));
    }

    #[test]
    fn test_undo_command_returns_undo_action() {
        use reovim_driver_command::{CommandResult, UndoAction};

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = UndoCommand.execute(&mut ctx, &args);
        assert!(result.is_undo_action());

        match result {
            CommandResult::UndoAction(UndoAction::Undo { count }) => {
                assert_eq!(count, 1); // Default count is 1
            }
            _ => panic!("Expected UndoAction::Undo"),
        }
    }

    #[test]
    fn test_redo_command_returns_undo_action() {
        use reovim_driver_command::{CommandResult, UndoAction};

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = RedoCommand.execute(&mut ctx, &args);
        assert!(result.is_undo_action());

        match result {
            CommandResult::UndoAction(UndoAction::Redo { count }) => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected UndoAction::Redo"),
        }
    }

    #[test]
    fn test_undo_command_respects_count() {
        use reovim_driver_command::{CommandResult, UndoAction};

        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(5));

        let result = UndoCommand.execute(&mut ctx, &args);

        match result {
            CommandResult::UndoAction(UndoAction::Undo { count }) => {
                assert_eq!(count, 5);
            }
            _ => panic!("Expected UndoAction::Undo"),
        }
    }

    #[test]
    fn test_redo_command_respects_count() {
        use reovim_driver_command::{CommandResult, UndoAction};

        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));

        let result = RedoCommand.execute(&mut ctx, &args);

        match result {
            CommandResult::UndoAction(UndoAction::Redo { count }) => {
                assert_eq!(count, 3);
            }
            _ => panic!("Expected UndoAction::Redo"),
        }
    }

    #[test]
    fn test_undo_command_zero_count_defaults_to_one() {
        use reovim_driver_command::{CommandResult, UndoAction};

        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(0));

        let result = UndoCommand.execute(&mut ctx, &args);

        // Zero count should be interpreted as 0 (caller's responsibility to handle)
        match result {
            CommandResult::UndoAction(UndoAction::Undo { count }) => {
                assert_eq!(count, 0);
            }
            _ => panic!("Expected UndoAction::Undo"),
        }
    }

    #[test]
    fn test_undo_commands_helper_returns_two_commands() {
        let cmds = undo_commands();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_undo_command_has_count_arg() {
        let cmd = UndoCommand;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_redo_command_has_count_arg() {
        let cmd = RedoCommand;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_undo_command_does_not_require_buffer_id() {
        // Undo commands return intent, they don't directly access the buffer
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        // Should NOT return an error, should return UndoAction
        let result = UndoCommand.execute(&mut ctx, &args);
        assert!(!result.is_error());
        assert!(result.is_undo_action());
    }

    #[test]
    fn test_redo_command_does_not_require_buffer_id() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = RedoCommand.execute(&mut ctx, &args);
        assert!(!result.is_error());
        assert!(result.is_undo_action());
    }

    // =========================================================================
    // Yank Command Tests
    // =========================================================================

    #[test]
    fn test_yank_line_command_id() {
        let cmd = YankLine;
        assert_eq!(cmd.id().name(), "yank-line");
    }

    #[test]
    fn test_yank_line_has_count_arg() {
        let cmd = YankLine;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_yank_line_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_yank_line_single_line() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_success());

        // Check register content
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "line one\n");
    }

    #[test]
    fn test_yank_line_count() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_success());

        // Check register content
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "line one\nline two\n");
    }

    #[test]
    fn test_yank_line_at_eof() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // Move to last line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(2, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(5)); // More than remaining lines

        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_success());

        // Should only yank the last line
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "line three\n");
    }

    #[test]
    fn test_yank_commands_count() {
        let cmds = yank_commands();
        assert_eq!(cmds.len(), 1);
    }

    // =========================================================================
    // Paste Command Tests
    // =========================================================================

    #[test]
    fn test_paste_after_command_id() {
        let cmd = PasteAfter;
        assert_eq!(cmd.id().name(), "paste-after");
    }

    #[test]
    fn test_paste_before_command_id() {
        let cmd = PasteBefore;
        assert_eq!(cmd.id().name(), "paste-before");
    }

    #[test]
    fn test_paste_after_has_count_arg() {
        let cmd = PasteAfter;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_paste_before_has_count_arg() {
        let cmd = PasteBefore;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_paste_after_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_before_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = PasteBefore.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_after_empty_register() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Register is empty by default
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_success()); // No-op, not error
    }

    #[test]
    fn test_paste_after_linewise() {
        let (mut ctx, buffer_id) = setup_buffer_context();

        // First yank a line
        {
            let mut args = CommandContext::new();
            args.set_buffer_id(buffer_id);
            YankLine.execute(&mut ctx, &args);
        }

        // Then paste after
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let line_count = buffer_read.line_count();
        let line1 = buffer_read.line(1).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(line_count, 4); // Original 3 + 1 pasted
        assert_eq!(line1.as_deref(), Some("line one")); // Pasted line
    }

    #[test]
    fn test_paste_before_linewise() {
        let (mut ctx, buffer_id) = setup_buffer_context();

        // First yank a line
        {
            let mut args = CommandContext::new();
            args.set_buffer_id(buffer_id);
            YankLine.execute(&mut ctx, &args);
        }

        // Then paste before
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let line_count = buffer_read.line_count();
        let line0 = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(line_count, 4); // Original 3 + 1 pasted
        assert_eq!(line0.as_deref(), Some("line one")); // Pasted line at top
    }

    #[test]
    fn test_paste_after_characterwise() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Set characterwise content in register
        ctx.registers
            .write()
            .set(RegisterContent::characterwise("XYZ"));

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - "XYZ" pasted after cursor (position 0)
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let content = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(content.as_deref(), Some("hXYZello world"));
    }

    #[test]
    fn test_paste_before_characterwise() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Set characterwise content in register
        ctx.registers
            .write()
            .set(RegisterContent::characterwise("XYZ"));

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - "XYZ" pasted at cursor (position 0)
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let content = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(content.as_deref(), Some("XYZhello world"));
    }

    #[test]
    fn test_paste_after_count() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        // Set characterwise content in register
        ctx.registers
            .write()
            .set(RegisterContent::characterwise("X"));

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - "XXX" pasted after cursor
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let content = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(content.as_deref(), Some("hXXXello"));
    }

    #[test]
    fn test_paste_commands_count() {
        let cmds = paste_commands();
        assert_eq!(cmds.len(), 2);
    }

    // =========================================================================
    // Change Command Tests
    // =========================================================================

    #[test]
    fn test_change_line_command_id() {
        let cmd = ChangeLine;
        assert_eq!(cmd.id().name(), "change-line");
    }

    #[test]
    fn test_change_to_eol_command_id() {
        let cmd = ChangeToEndOfLine;
        assert_eq!(cmd.id().name(), "change-to-eol");
    }

    #[test]
    fn test_change_line_has_count_arg() {
        let cmd = ChangeLine;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_change_line_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = ChangeLine.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_change_to_eol_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_change_line_single_line() {
        let (mut ctx, _) = setup_buffer_context();

        // Replace buffer with single line content
        let buffer = Buffer::from_string("hello world");
        let new_buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(new_buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - line should be empty
        let buffer = ctx.buffers.get(new_buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "");

        // Check register - should have deleted text as linewise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "hello world\n");
    }

    #[test]
    fn test_change_line_multi_line() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // setup_buffer_context gives us "line one\nline two\nline three"

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));
        let result = ChangeLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check register - should have deleted text as linewise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert!(content.text.contains("line one"));
        assert!(content.text.contains("line two"));
    }

    #[test]
    fn test_change_line_empty_buffer() {
        let mut ctx = create_test_context();
        let buffer = Buffer::new();
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);
        // Should succeed (enters insert mode on empty buffer)
        assert!(result.is_success());
    }

    #[test]
    fn test_change_to_eol_middle_of_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Position cursor at column 6 (at 'w')
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 6));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - should have "hello "
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "hello ");

        // Check register - should have "world" as characterwise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(!content.is_linewise());
        assert_eq!(content.text, "world");
    }

    #[test]
    fn test_change_to_eol_at_start() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - should be empty
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "");

        // Check register - should have "hello world" as characterwise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(!content.is_linewise());
        assert_eq!(content.text, "hello world");
    }

    #[test]
    fn test_change_to_eol_at_end_of_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        // Position cursor at end of line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 5));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        // Should succeed but be a no-op (just enters insert mode)
        assert!(result.is_success());

        // Buffer should be unchanged
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "hello");
    }

    #[test]
    fn test_change_commands_count() {
        let cmds = change_commands();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_all_commands_includes_change() {
        let cmds = all_commands();
        let has_change_line = cmds.iter().any(|c| c.id().name() == "change-line");
        let has_change_to_eol = cmds.iter().any(|c| c.id().name() == "change-to-eol");
        assert!(has_change_line);
        assert!(has_change_to_eol);
    }

    #[test]
    fn test_change_line_returns_edit_action() {
        use reovim_driver_command::CommandResult;

        let (mut ctx, buffer_id) = setup_buffer_context();

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);

        // Verify EditAction returned for undo integration
        assert!(result.is_edit_action());
        if let CommandResult::EditAction(action) = result {
            // EditAction should have non-empty edits
            assert!(!action.edits.is_empty());
            // Verify buffer_id is correct
            assert_eq!(action.buffer_id, buffer_id);
        } else {
            panic!("Expected EditAction result");
        }
    }

    #[test]
    fn test_change_to_eol_returns_edit_action() {
        use reovim_driver_command::CommandResult;

        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);

        // Verify EditAction returned for undo integration
        assert!(result.is_edit_action());
        if let CommandResult::EditAction(action) = result {
            // EditAction should have non-empty edits
            assert!(!action.edits.is_empty());
            // Verify buffer_id is correct
            assert_eq!(action.buffer_id, buffer_id);
        } else {
            panic!("Expected EditAction result");
        }
    }

    #[test]
    fn test_change_line_cursor_position_after() {
        use reovim_driver_command::CommandResult;

        let (mut ctx, buffer_id) = setup_buffer_context();

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);

        // After cc, cursor should be at column 0
        if let CommandResult::EditAction(action) = result {
            assert_eq!(action.cursor_after.column, 0);
        } else {
            panic!("Expected EditAction result");
        }
    }

    // =========================================================================
    // Replace Char Tests
    // =========================================================================

    #[test]
    fn test_replace_char_start_command_id() {
        let cmd = ReplaceCharStart;
        assert_eq!(cmd.id().name(), "replace-char-start");
    }

    #[test]
    fn test_replace_char_start_has_count_arg() {
        let cmd = ReplaceCharStart;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
    }

    #[test]
    fn test_replace_char_start_returns_waiting_for_char() {
        let (mut ctx, _buffer_id) = setup_buffer_context();
        let args = CommandContext::new();
        let result = ReplaceCharStart.execute(&mut ctx, &args);

        assert!(result.is_waiting_for_char());
    }

    #[test]
    fn test_replace_char_start_with_count() {
        use reovim_driver_command::{CharWaitOp, CommandResult};

        let (mut ctx, _buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        let result = ReplaceCharStart.execute(&mut ctx, &args);

        if let CommandResult::WaitingForChar(char_ctx) = result {
            assert_eq!(char_ctx.op_type, CharWaitOp::ReplaceChar);
            assert_eq!(char_ctx.count, Some(3));
        } else {
            panic!("Expected WaitingForChar result");
        }
    }

    #[test]
    fn test_replace_char_start_default_count_is_one() {
        use reovim_driver_command::CommandResult;

        let (mut ctx, _buffer_id) = setup_buffer_context();
        let args = CommandContext::new();
        let result = ReplaceCharStart.execute(&mut ctx, &args);

        if let CommandResult::WaitingForChar(char_ctx) = result {
            assert_eq!(char_ctx.count, Some(1));
        } else {
            panic!("Expected WaitingForChar result");
        }
    }

    // =========================================================================
    // Repeat Dot Tests
    // =========================================================================

    #[test]
    fn test_repeat_dot_command_id() {
        let cmd = RepeatDot;
        assert_eq!(cmd.id().name(), "repeat-dot");
    }

    #[test]
    fn test_repeat_dot_has_count_arg() {
        let cmd = RepeatDot;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
    }

    #[test]
    fn test_repeat_dot_returns_repeat_action() {
        let (mut ctx, _buffer_id) = setup_buffer_context();
        let args = CommandContext::new();
        let result = RepeatDot.execute(&mut ctx, &args);

        assert!(result.is_repeat_action());
    }
}
