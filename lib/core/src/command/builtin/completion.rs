//! Completion-related commands

use crate::command::traits::{
    CommandResult, CommandTrait, CompletionAction, DeferredAction, ExecutionContext,
};
use std::any::Any;

/// Trigger completion at current cursor position
#[derive(Debug, Clone)]
pub struct CompletionTriggerCommand;

impl CommandTrait for CompletionTriggerCommand {
    fn name(&self) -> &'static str {
        "completion_trigger"
    }

    fn description(&self) -> &'static str {
        "Trigger completion at cursor position"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Completion(CompletionAction::Trigger))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Select next completion item
#[derive(Debug, Clone)]
pub struct CompletionNextCommand;

impl CommandTrait for CompletionNextCommand {
    fn name(&self) -> &'static str {
        "completion_next"
    }

    fn description(&self) -> &'static str {
        "Select next completion item"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Completion(CompletionAction::SelectNext))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Select previous completion item
#[derive(Debug, Clone)]
pub struct CompletionPrevCommand;

impl CommandTrait for CompletionPrevCommand {
    fn name(&self) -> &'static str {
        "completion_prev"
    }

    fn description(&self) -> &'static str {
        "Select previous completion item"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Completion(CompletionAction::SelectPrev))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Confirm and insert selected completion
#[derive(Debug, Clone)]
pub struct CompletionConfirmCommand;

impl CommandTrait for CompletionConfirmCommand {
    fn name(&self) -> &'static str {
        "completion_confirm"
    }

    fn description(&self) -> &'static str {
        "Confirm and insert selected completion"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Completion(CompletionAction::Confirm))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Dismiss completion popup
#[derive(Debug, Clone)]
pub struct CompletionDismissCommand;

impl CommandTrait for CompletionDismissCommand {
    fn name(&self) -> &'static str {
        "completion_dismiss"
    }

    fn description(&self) -> &'static str {
        "Dismiss completion popup"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Completion(CompletionAction::Dismiss))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
