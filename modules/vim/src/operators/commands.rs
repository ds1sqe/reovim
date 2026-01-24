//! Operator command wrappers for Epic #415.
//!
//! These `CommandHandler` implementations wrap the `Operator` trait,
//! reading range information from `CommandContext` and delegating to
//! the appropriate operator.
//!
//! # Architecture
//!
//! When the runner receives `PopResult::ExecuteCommand`, it executes
//! the command with the provided arguments. This module provides the
//! command handlers that are registered with IDs like `vim:delete`
//! and perform the actual operation.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{SessionRuntime, TransitionContext, api::ModeApi},
    reovim_kernel::api::v1::{CommandId, Position},
};

use {
    super::{DeleteOperator, Operator, OperatorContext, Range, YankOperator},
    crate::ids::MODULE,
};

// =============================================================================
// Delete Command (vim:delete)
// =============================================================================

/// Delete operator command - wraps `DeleteOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:delete`. It reads the range from the
/// command context and delegates to `DeleteOperator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteCommand;

impl Command for DeleteCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "delete")
    }

    fn description(&self) -> &'static str {
        "Delete text in range"
    }
}

impl CommandHandler for DeleteCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_operator(&DeleteOperator, runtime, args)
    }
}

// =============================================================================
// Yank Command (vim:yank)
// =============================================================================

/// Yank operator command - wraps `YankOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:yank`. It reads the range from the
/// command context and delegates to `YankOperator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct YankCommand;

impl Command for YankCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "yank")
    }

    fn description(&self) -> &'static str {
        "Yank text in range to register"
    }
}

impl CommandHandler for YankCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_operator(&YankOperator, runtime, args)
    }
}

// =============================================================================
// Change Command (vim:change)
// =============================================================================

/// Change operator command - wraps `ChangeOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:change`. It reads the range from the
/// command context, deletes the text, and transitions to insert mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeCommand;

impl Command for ChangeCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "change")
    }

    fn description(&self) -> &'static str {
        "Change text in range (delete and enter insert)"
    }
}

impl CommandHandler for ChangeCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        use {super::ChangeOperator, crate::modes::VimMode};

        // Execute the change operator (delete text)
        let result = execute_operator(&ChangeOperator, runtime, args);

        // If successful, enter insert mode (change = delete + insert)
        if matches!(result, CommandResult::Success) {
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
        }

        result
    }
}

// =============================================================================
// Helper Function
// =============================================================================

/// Execute an operator with range information from CommandContext.
///
/// This function bridges the `CommandHandler` interface to the `Operator` trait:
/// 1. Extracts range_start, range_end, linewise from args
/// 2. Builds an `OperatorContext` with kernel access
/// 3. Calls `operator.execute()`
fn execute_operator(
    operator: &dyn Operator,
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
) -> CommandResult {
    // Get buffer ID
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get range from context (Epic #415 Phase 5)
    let (start_line, start_col) = args.range_start().unwrap_or((0, 0));
    let (end_line, end_col) = args.range_end().unwrap_or((0, 0));
    let linewise = args.is_linewise();

    let start = Position::new(start_line, start_col);
    let end = Position::new(end_line, end_col);

    // Build range
    let range = if linewise {
        Range::linewise(start, end)
    } else {
        Range::new(start, end)
    };

    // Get count and register
    let count = args.count().unwrap_or(1);
    let register = args.register();

    // Build operator context using runtime's kernel (uses interior mutability)
    let mut op_ctx = OperatorContext {
        kernel: runtime.kernel(),
        buffer_id,
        register,
        count,
    };

    // Execute operator
    match operator.execute(&mut op_ctx, range) {
        Ok(()) => CommandResult::Success,
        Err(e) => CommandResult::error(&e.to_string()),
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all operator command handlers.
#[must_use]
pub fn operator_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteCommand),
        Box::new(YankCommand),
        Box::new(ChangeCommand),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_command_id() {
        let cmd = DeleteCommand;
        assert_eq!(cmd.id().module().as_str(), "vim");
        assert_eq!(cmd.id().name(), "delete");
    }

    #[test]
    fn test_yank_command_id() {
        let cmd = YankCommand;
        assert_eq!(cmd.id().module().as_str(), "vim");
        assert_eq!(cmd.id().name(), "yank");
    }

    #[test]
    fn test_change_command_id() {
        let cmd = ChangeCommand;
        assert_eq!(cmd.id().module().as_str(), "vim");
        assert_eq!(cmd.id().name(), "change");
    }

    #[test]
    fn test_operator_commands_count() {
        let cmds = operator_commands();
        assert_eq!(cmds.len(), 3);
    }
}
