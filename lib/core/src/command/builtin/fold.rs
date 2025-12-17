//! Code folding commands

use crate::command::traits::{CommandResult, CommandTrait, DeferredAction, ExecutionContext, FoldAction};
use std::any::Any;

/// Toggle fold at cursor line (za)
#[derive(Debug, Clone)]
pub struct FoldToggleCommand;

impl CommandTrait for FoldToggleCommand {
    fn name(&self) -> &'static str {
        "fold_toggle"
    }

    fn description(&self) -> &'static str {
        "Toggle fold at cursor line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Fold(FoldAction::Toggle))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open fold at cursor line (zo)
#[derive(Debug, Clone)]
pub struct FoldOpenCommand;

impl CommandTrait for FoldOpenCommand {
    fn name(&self) -> &'static str {
        "fold_open"
    }

    fn description(&self) -> &'static str {
        "Open fold at cursor line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Fold(FoldAction::Open))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close fold at cursor line (zc)
#[derive(Debug, Clone)]
pub struct FoldCloseCommand;

impl CommandTrait for FoldCloseCommand {
    fn name(&self) -> &'static str {
        "fold_close"
    }

    fn description(&self) -> &'static str {
        "Close fold at cursor line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Fold(FoldAction::Close))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open all folds in buffer (zR)
#[derive(Debug, Clone)]
pub struct FoldOpenAllCommand;

impl CommandTrait for FoldOpenAllCommand {
    fn name(&self) -> &'static str {
        "fold_open_all"
    }

    fn description(&self) -> &'static str {
        "Open all folds in buffer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Fold(FoldAction::OpenAll))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close all folds in buffer (zM)
#[derive(Debug, Clone)]
pub struct FoldCloseAllCommand;

impl CommandTrait for FoldCloseAllCommand {
    fn name(&self) -> &'static str {
        "fold_close_all"
    }

    fn description(&self) -> &'static str {
        "Close all folds in buffer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Fold(FoldAction::CloseAll))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
