//! Editor commands - cursor movement and mode switching.
//!
//! This module provides the basic commands for editor operation:
//! - Cursor movement: up, down, left, right
//! - Mode switching: enter insert, exit to normal

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext, events::ModeChanged},
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // TODO: Implement actual cursor movement in Phase 4
        // For now, just return success to verify the wiring works
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // TODO: Implement actual cursor movement in Phase 4
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // TODO: Implement actual cursor movement in Phase 4
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // TODO: Implement actual cursor movement in Phase 4
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(cmds.len(), 7);
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

    #[test]
    fn test_command_execute_returns_success() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        // All commands should return Success for now
        assert_eq!(CursorUp.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(CursorDown.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(CursorLeft.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(CursorRight.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(EnterInsertMode.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(EnterInsertModeAppend.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(ExitToNormal.execute(&mut ctx, &args), CommandResult::Success);
    }
}
