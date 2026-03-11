//! Enter-operator commands for operator-pending mode.
//!
//! Provides commands that enter operator-pending mode with a specific operator:
//! - `EnterDeleteOperator` (d)
//! - `EnterYankOperator` (y)
//! - `EnterChangeOperator` (c)
//! - `EnterIndentOperator` (>)
//! - `EnterDedentOperator` (<)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

/// Enter delete operator-pending mode (d).
///
/// Sets the pending operator to "delete" and enters operator-pending mode,
/// waiting for a motion to determine the range to delete.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterDeleteOperator;

impl Command for EnterDeleteOperator {
    fn id(&self) -> CommandId {
        ids::ENTER_DELETE_OPERATOR
    }

    fn description(&self) -> &'static str {
        "Enter delete operator-pending mode"
    }
}

impl CommandHandler for EnterDeleteOperator {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will set pending operator to "delete" and enter operator-pending mode
        let _ = args.register(); // Suppress unused warning
        CommandResult::Success
    }
}

/// Enter yank operator-pending mode (y).
///
/// Sets the pending operator to "yank" and enters operator-pending mode,
/// waiting for a motion to determine the range to yank.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterYankOperator;

impl Command for EnterYankOperator {
    fn id(&self) -> CommandId {
        ids::ENTER_YANK_OPERATOR
    }

    fn description(&self) -> &'static str {
        "Enter yank operator-pending mode"
    }
}

impl CommandHandler for EnterYankOperator {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will set pending operator to "yank" and enter operator-pending mode
        let _ = args.register(); // Suppress unused warning
        CommandResult::Success
    }
}

/// Enter change operator-pending mode (c).
///
/// Sets the pending operator to "change" and enters operator-pending mode,
/// waiting for a motion to determine the range to change. After deletion,
/// the editor enters insert mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterChangeOperator;

impl Command for EnterChangeOperator {
    fn id(&self) -> CommandId {
        ids::ENTER_CHANGE_OPERATOR
    }

    fn description(&self) -> &'static str {
        "Enter change operator-pending mode"
    }
}

impl CommandHandler for EnterChangeOperator {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will set pending operator to "change" and enter operator-pending mode
        let _ = args.register(); // Suppress unused warning
        CommandResult::Success
    }
}

/// Enter indent operator-pending mode (>).
///
/// Sets the pending operator to "indent" and enters operator-pending mode,
/// waiting for a motion to determine the range to indent.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterIndentOperator;

impl Command for EnterIndentOperator {
    fn id(&self) -> CommandId {
        ids::ENTER_INDENT_OPERATOR
    }

    fn description(&self) -> &'static str {
        "Enter indent operator-pending mode"
    }
}

impl CommandHandler for EnterIndentOperator {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will set pending operator to "indent" and enter operator-pending mode
        CommandResult::Success
    }
}

/// Enter dedent operator-pending mode (<).
///
/// Sets the pending operator to "dedent" and enters operator-pending mode,
/// waiting for a motion to determine the range to dedent.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterDedentOperator;

impl Command for EnterDedentOperator {
    fn id(&self) -> CommandId {
        ids::ENTER_DEDENT_OPERATOR
    }

    fn description(&self) -> &'static str {
        "Enter dedent operator-pending mode"
    }
}

impl CommandHandler for EnterDedentOperator {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will set pending operator to "dedent" and enter operator-pending mode
        CommandResult::Success
    }
}

