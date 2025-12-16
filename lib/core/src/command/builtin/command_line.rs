//! Command line mode commands

use crate::command::traits::{
    CommandLineAction, CommandResult, CommandTrait, DeferredAction, ExecutionContext,
};
use std::any::Any;

/// Insert character into command line
#[derive(Debug, Clone)]
pub struct CommandLineCharCommand {
    pub char: char,
}

impl CommandLineCharCommand {
    #[must_use]
    pub const fn new(c: char) -> Self {
        Self { char: c }
    }
}

impl CommandTrait for CommandLineCharCommand {
    fn name(&self) -> &'static str {
        "command_line_char"
    }

    fn description(&self) -> &'static str {
        "Insert character into command line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::CommandLine(CommandLineAction::InsertChar(
            self.char,
        )))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Delete character in command line (backspace)
#[derive(Debug, Clone)]
pub struct CommandLineBackspaceCommand;

impl CommandTrait for CommandLineBackspaceCommand {
    fn name(&self) -> &'static str {
        "command_line_backspace"
    }

    fn description(&self) -> &'static str {
        "Delete character in command line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::CommandLine(CommandLineAction::Backspace))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Execute command line
#[derive(Debug, Clone)]
pub struct CommandLineExecuteCommand;

impl CommandTrait for CommandLineExecuteCommand {
    fn name(&self) -> &'static str {
        "command_line_execute"
    }

    fn description(&self) -> &'static str {
        "Execute command line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::CommandLine(CommandLineAction::Execute))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cancel command line
#[derive(Debug, Clone)]
pub struct CommandLineCancelCommand;

impl CommandTrait for CommandLineCancelCommand {
    fn name(&self) -> &'static str {
        "command_line_cancel"
    }

    fn description(&self) -> &'static str {
        "Cancel command line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::CommandLine(CommandLineAction::Cancel))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
