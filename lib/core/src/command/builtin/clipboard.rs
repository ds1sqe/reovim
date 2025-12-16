//! Clipboard commands

use crate::command::traits::{CommandResult, CommandTrait, DeferredAction, ExecutionContext};
use std::any::Any;

/// Paste after cursor
#[derive(Debug, Clone)]
pub struct PasteCommand;

impl CommandTrait for PasteCommand {
    fn name(&self) -> &'static str {
        "paste"
    }

    fn description(&self) -> &'static str {
        "Paste clipboard content after cursor"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Paste { before: false, register: None })
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Paste before cursor
#[derive(Debug, Clone)]
pub struct PasteBeforeCommand;

impl CommandTrait for PasteBeforeCommand {
    fn name(&self) -> &'static str {
        "paste_before"
    }

    fn description(&self) -> &'static str {
        "Paste clipboard content before cursor"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Paste { before: true, register: None })
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
