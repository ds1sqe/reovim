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
    reovim_kernel::api::v1::{CommandId, KernelContext},
};

use super::super::mode::EDITOR_MODULE;

/// Enter delete operator-pending mode (d).
///
/// Sets the pending operator to "delete" and enters operator-pending mode,
/// waiting for a motion to determine the range to delete.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterDeleteOperator;

impl Command for EnterDeleteOperator {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-delete-operator")
    }

    fn description(&self) -> &'static str {
        "Enter delete operator-pending mode"
    }
}

impl CommandHandler for EnterDeleteOperator {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let register = args.register();
        CommandResult::enter_operator_pending("delete", register)
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
        CommandId::new(EDITOR_MODULE, "enter-yank-operator")
    }

    fn description(&self) -> &'static str {
        "Enter yank operator-pending mode"
    }
}

impl CommandHandler for EnterYankOperator {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let register = args.register();
        CommandResult::enter_operator_pending("yank", register)
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
        CommandId::new(EDITOR_MODULE, "enter-change-operator")
    }

    fn description(&self) -> &'static str {
        "Enter change operator-pending mode"
    }
}

impl CommandHandler for EnterChangeOperator {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let register = args.register();
        CommandResult::enter_operator_pending("change", register)
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
        CommandId::new(EDITOR_MODULE, "enter-indent-operator")
    }

    fn description(&self) -> &'static str {
        "Enter indent operator-pending mode"
    }
}

impl CommandHandler for EnterIndentOperator {
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::enter_operator_pending("indent", None)
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
        CommandId::new(EDITOR_MODULE, "enter-dedent-operator")
    }

    fn description(&self) -> &'static str {
        "Enter dedent operator-pending mode"
    }
}

impl CommandHandler for EnterDedentOperator {
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::enter_operator_pending("dedent", None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_delete_operator_id() {
        let cmd = EnterDeleteOperator;
        assert_eq!(cmd.id().name(), "enter-delete-operator");
    }

    #[test]
    fn test_enter_yank_operator_id() {
        let cmd = EnterYankOperator;
        assert_eq!(cmd.id().name(), "enter-yank-operator");
    }

    #[test]
    fn test_enter_change_operator_id() {
        let cmd = EnterChangeOperator;
        assert_eq!(cmd.id().name(), "enter-change-operator");
    }

    #[test]
    fn test_enter_indent_operator_id() {
        let cmd = EnterIndentOperator;
        assert_eq!(cmd.id().name(), "enter-indent-operator");
    }

    #[test]
    fn test_enter_dedent_operator_id() {
        let cmd = EnterDedentOperator;
        assert_eq!(cmd.id().name(), "enter-dedent-operator");
    }

    #[test]
    fn test_enter_delete_operator_returns_enter_operator_pending() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = EnterDeleteOperator.execute(&mut ctx, &args);

        assert!(result.is_enter_operator_pending());
        if let CommandResult::EnterOperatorPending {
            operator_id,
            register,
        } = result
        {
            assert_eq!(operator_id, "delete");
            assert_eq!(register, None);
        } else {
            panic!("Expected EnterOperatorPending result");
        }
    }

    #[test]
    fn test_enter_yank_operator_returns_enter_operator_pending() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = EnterYankOperator.execute(&mut ctx, &args);

        assert!(result.is_enter_operator_pending());
        if let CommandResult::EnterOperatorPending {
            operator_id,
            register,
        } = result
        {
            assert_eq!(operator_id, "yank");
            assert_eq!(register, None);
        } else {
            panic!("Expected EnterOperatorPending result");
        }
    }

    #[test]
    fn test_enter_change_operator_returns_enter_operator_pending() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = EnterChangeOperator.execute(&mut ctx, &args);

        assert!(result.is_enter_operator_pending());
        if let CommandResult::EnterOperatorPending {
            operator_id,
            register,
        } = result
        {
            assert_eq!(operator_id, "change");
            assert_eq!(register, None);
        } else {
            panic!("Expected EnterOperatorPending result");
        }
    }

    #[test]
    fn test_enter_delete_operator_with_register() {
        use reovim_driver_command::ArgValue;

        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("register", ArgValue::Register('a'));
        let result = EnterDeleteOperator.execute(&mut ctx, &args);

        if let CommandResult::EnterOperatorPending {
            operator_id,
            register,
        } = result
        {
            assert_eq!(operator_id, "delete");
            assert_eq!(register, Some('a'));
        } else {
            panic!("Expected EnterOperatorPending result");
        }
    }
}
