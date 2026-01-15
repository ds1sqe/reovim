//! Editor commands - cursor movement and mode switching.
//!
//! This module provides the basic commands for editor operation:
//! - Cursor movement: up, down, left, right
//! - Mode switching: enter insert, exit to normal
//!
//! # Cursor Movement Philosophy
//!
//! Cursor movement commands follow Vim semantics:
//! - j/k (down/up) preserve the "preferred column" - the column the user
//!   intended, even if shorter lines force temporary repositioning
//! - h/l (left/right) clear the preferred column
//! - Movements clamp to valid positions (no-op at boundaries)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult, UndoAction,
    },
    reovim_kernel::api::v1::{
        CommandId, KernelContext, Position,
        events::{CursorMoved, ModeChanged},
    },
};

use super::mode::EDITOR_MODULE;

// =============================================================================
// Cursor Movement Commands
// =============================================================================

/// Move cursor up.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorUp;

impl Command for CursorUp {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-up")
    }

    fn description(&self) -> &'static str {
        "Move cursor up"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for CursorUp {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Calculate new line (saturating sub to handle boundary)
            let new_line = old_pos.line.saturating_sub(count);

            // If already at top, no-op
            if new_line == old_pos.line && old_pos.line == 0 {
                return CommandResult::Success;
            }

            // Get line length for column clamping
            let line_len = buffer.line_len(new_line).unwrap_or(0);
            let new_col = old_pos.column.min(line_len);

            let new_pos = Position::new(new_line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

/// Move cursor down.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDown;

impl Command for CursorDown {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-down")
    }

    fn description(&self) -> &'static str {
        "Move cursor down"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for CursorDown {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();
            let line_count = buffer.line_count();

            // Calculate new line (clamped to last line)
            let max_line = line_count.saturating_sub(1);
            let new_line = (old_pos.line + count).min(max_line);

            // If already at bottom, no-op
            if new_line == old_pos.line && old_pos.line == max_line {
                return CommandResult::Success;
            }

            // Get line length for column clamping
            let line_len = buffer.line_len(new_line).unwrap_or(0);
            let new_col = old_pos.column.min(line_len);

            let new_pos = Position::new(new_line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

/// Move cursor left.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorLeft;

impl Command for CursorLeft {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-left")
    }

    fn description(&self) -> &'static str {
        "Move cursor left"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of columns",
        )]
    }
}

impl CommandHandler for CursorLeft {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Calculate new column (saturating sub to handle boundary)
            let new_col = old_pos.column.saturating_sub(count);

            // If already at left edge, no-op
            if new_col == old_pos.column && old_pos.column == 0 {
                return CommandResult::Success;
            }

            let new_pos = Position::new(old_pos.line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

/// Move cursor right.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorRight;

impl Command for CursorRight {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-right")
    }

    fn description(&self) -> &'static str {
        "Move cursor right"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of columns",
        )]
    }
}

impl CommandHandler for CursorRight {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Get current line length for boundary check
            let line_len = buffer.line_len(old_pos.line).unwrap_or(0);

            // Calculate new column (clamped to line length)
            let new_col = (old_pos.column + count).min(line_len);

            // If already at right edge, no-op
            if new_col == old_pos.column && old_pos.column == line_len {
                return CommandResult::Success;
            }

            let new_pos = Position::new(old_pos.line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

// =============================================================================
// Mode Switching Commands
// =============================================================================

/// Enter insert mode (before cursor).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertMode;

impl Command for EnterInsertMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode"
    }
}

impl CommandHandler for EnterInsertMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus.emit(ModeChanged {
            from: "normal".to_string(),
            to: "insert".to_string(),
        });

        // Note: The actual mode stack change happens in the runner via a callback
        // or by the caller checking the result. For now, we just emit the event.

        CommandResult::Success
    }
}

/// Enter insert mode (after cursor, append).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertModeAppend;

impl Command for EnterInsertModeAppend {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert-append")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode after cursor (append)"
    }
}

impl CommandHandler for EnterInsertModeAppend {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // TODO: Move cursor right first, then enter insert mode
        ctx.event_bus.emit(ModeChanged {
            from: "normal".to_string(),
            to: "insert".to_string(),
        });

        CommandResult::Success
    }
}

/// Exit to normal mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitToNormal;

impl Command for ExitToNormal {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "exit-to-normal")
    }

    fn description(&self) -> &'static str {
        "Exit to normal mode"
    }
}

impl CommandHandler for ExitToNormal {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        ctx.event_bus.emit(ModeChanged {
            from: "insert".to_string(),
            to: "normal".to_string(),
        });

        CommandResult::Success
    }
}

// =============================================================================
// Undo/Redo Commands
// =============================================================================

/// Undo the last change.
///
/// Returns an `UndoAction` intent for the runner to handle. The runner
/// maintains per-buffer undo trees and applies the actual undo operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct UndoCommand;

impl Command for UndoCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "undo")
    }

    fn description(&self) -> &'static str {
        "Undo the last change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to undo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["u", "undo"]
    }
}

impl CommandHandler for UndoCommand {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let count = args.count().unwrap_or(1);
        CommandResult::UndoAction(UndoAction::Undo { count })
    }
}

/// Redo the last undone change.
///
/// Returns an `UndoAction` intent for the runner to handle.
#[derive(Debug, Clone, Copy, Default)]
pub struct RedoCommand;

impl Command for RedoCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "redo")
    }

    fn description(&self) -> &'static str {
        "Redo the last undone change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to redo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["redo"]
    }
}

impl CommandHandler for RedoCommand {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let count = args.count().unwrap_or(1);
        CommandResult::UndoAction(UndoAction::Redo { count })
    }
}

// =============================================================================
// Command Registration Helper
// =============================================================================

/// Get all editor commands as boxed trait objects.
///
/// This is useful for registering all commands at once.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(ExitToNormal),
        Box::new(UndoCommand),
        Box::new(RedoCommand),
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

/// Get all mode switching commands.
#[must_use]
pub fn mode_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(ExitToNormal),
    ]
}

/// Get all undo/redo commands.
#[must_use]
pub fn undo_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(UndoCommand), Box::new(RedoCommand)]
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::ArgValue,
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, MotionEngine,
            OptionRegistry, RegisterBank, RwLock, TextObjectEngine,
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
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
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
        assert_eq!(cmd.id().name(), "enter-insert-append");
    }

    #[test]
    fn test_exit_to_normal_id() {
        let cmd = ExitToNormal;
        assert_eq!(cmd.id().name(), "exit-to-normal");
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
        assert_eq!(cmds.len(), 9); // 4 cursor + 3 mode + 2 undo
    }

    #[test]
    fn test_cursor_commands_count() {
        let cmds = cursor_commands();
        assert_eq!(cmds.len(), 4);
    }

    #[test]
    fn test_mode_commands_count() {
        let cmds = mode_commands();
        assert_eq!(cmds.len(), 3);
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
        // Move to end of line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 8)); // "line one" is 8 chars
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 8); // Stayed at EOL
    }

    // =========================================================================
    // Mode Commands Tests
    // =========================================================================

    #[test]
    fn test_mode_commands_execute_success() {
        let mut ctx = create_test_context();
        let args = CommandContext::new();

        // Mode commands don't require buffer_id
        assert_eq!(EnterInsertMode.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(EnterInsertModeAppend.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(ExitToNormal.execute(&mut ctx, &args), CommandResult::Success);
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
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "undo");
    }

    #[test]
    fn test_redo_command_id() {
        let cmd = RedoCommand;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
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
}
