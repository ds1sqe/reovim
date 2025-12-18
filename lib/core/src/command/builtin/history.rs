//! Undo and redo commands

use {
    crate::command::traits::{CommandResult, CommandTrait, ExecutionContext},
    std::any::Any,
};

/// Undo last change
#[derive(Debug, Clone)]
pub struct UndoCommand;

impl CommandTrait for UndoCommand {
    fn name(&self) -> &'static str {
        "undo"
    }

    fn description(&self) -> &'static str {
        "Undo last change"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        if ctx.buffer.apply_undo() {
            CommandResult::NeedsRender
        } else {
            CommandResult::Success
        }
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Redo last undone change
#[derive(Debug, Clone)]
pub struct RedoCommand;

impl CommandTrait for RedoCommand {
    fn name(&self) -> &'static str {
        "redo"
    }

    fn description(&self) -> &'static str {
        "Redo last undone change"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        if ctx.buffer.apply_redo() {
            CommandResult::NeedsRender
        } else {
            CommandResult::Success
        }
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
