//! System commands

use crate::command::traits::{CommandResult, CommandTrait, ExecutionContext};
use std::any::Any;

/// Quit the editor
#[derive(Debug, Clone)]
pub struct QuitCommand;

impl CommandTrait for QuitCommand {
    fn name(&self) -> &'static str {
        "quit"
    }

    fn description(&self) -> &'static str {
        "Quit the editor"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::Quit
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// No operation (do nothing)
#[derive(Debug, Clone)]
pub struct NoopCommand;

impl CommandTrait for NoopCommand {
    fn name(&self) -> &'static str {
        "noop"
    }

    fn description(&self) -> &'static str {
        "Do nothing"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::Success
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
