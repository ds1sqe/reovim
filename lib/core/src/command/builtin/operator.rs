//! Operator commands for delete, yank, change operations

use {
    crate::{
        command::traits::{CommandResult, CommandTrait, ExecutionContext},
        modd::{ModeState, OperatorType},
    },
    std::any::Any,
};

/// Enter operator-pending mode for delete (d)
#[derive(Debug, Clone)]
pub struct EnterDeleteOperatorCommand;

impl CommandTrait for EnterDeleteOperatorCommand {
    fn name(&self) -> &'static str {
        "enter_delete_operator"
    }

    fn description(&self) -> &'static str {
        "Enter delete operator-pending mode"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::operator_pending(OperatorType::Delete, ctx.count))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter operator-pending mode for yank (y)
#[derive(Debug, Clone)]
pub struct EnterYankOperatorCommand;

impl CommandTrait for EnterYankOperatorCommand {
    fn name(&self) -> &'static str {
        "enter_yank_operator"
    }

    fn description(&self) -> &'static str {
        "Enter yank operator-pending mode"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::operator_pending(OperatorType::Yank, ctx.count))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter operator-pending mode for change (c)
#[derive(Debug, Clone)]
pub struct EnterChangeOperatorCommand;

impl CommandTrait for EnterChangeOperatorCommand {
    fn name(&self) -> &'static str {
        "enter_change_operator"
    }

    fn description(&self) -> &'static str {
        "Enter change operator-pending mode"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::operator_pending(OperatorType::Change, ctx.count))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
