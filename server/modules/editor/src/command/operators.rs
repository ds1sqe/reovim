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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will set pending operator to "dedent" and enter operator-pending mode
        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::CommandContext,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, WindowLayout, api::CommandExecutor,
        },
        reovim_kernel::api::v1::{
            CommandId as KernelCommandId, KernelContext, ModeId, ModeStack, ModuleId,
        },
    };

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    struct StubExecutor;

    impl CommandExecutor for StubExecutor {
        fn execute(
            &self,
            _cmd: &KernelCommandId,
            _ctx: &CommandContext,
            _kernel: &KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
        }
    }

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
    fn test_enter_delete_operator_returns_success() {
        // Commands now return Success - actual operator-pending logic
        // will be handled by the vim resolver via SessionRuntime (see #394)
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = EnterDeleteOperator.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_enter_yank_operator_returns_success() {
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = EnterYankOperator.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_enter_change_operator_returns_success() {
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = EnterChangeOperator.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_enter_indent_operator_returns_success() {
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = EnterIndentOperator.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_enter_dedent_operator_returns_success() {
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let kernel = KernelContext::default();
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = EnterDedentOperator.execute(&mut runtime, &args);
        assert!(result.is_success());
    }
}
