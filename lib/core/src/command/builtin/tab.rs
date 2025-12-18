//! Tab management commands

use {
    crate::command::traits::{
        CommandResult, CommandTrait, DeferredAction, ExecutionContext, TabAction,
    },
    std::any::Any,
};

/// Create new tab (:tabnew)
#[derive(Debug, Clone)]
pub struct TabNewCommand;

impl CommandTrait for TabNewCommand {
    fn name(&self) -> &'static str {
        "tab_new"
    }

    fn description(&self) -> &'static str {
        "Create new tab"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Tab(TabAction::New { filename: None }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close current tab (:tabclose)
#[derive(Debug, Clone)]
pub struct TabCloseCommand;

impl CommandTrait for TabCloseCommand {
    fn name(&self) -> &'static str {
        "tab_close"
    }

    fn description(&self) -> &'static str {
        "Close current tab"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Tab(TabAction::Close))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Switch to next tab (gt)
#[derive(Debug, Clone)]
pub struct TabNextCommand;

impl CommandTrait for TabNextCommand {
    fn name(&self) -> &'static str {
        "tab_next"
    }

    fn description(&self) -> &'static str {
        "Switch to next tab"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Tab(TabAction::Next))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Switch to previous tab (gT)
#[derive(Debug, Clone)]
pub struct TabPrevCommand;

impl CommandTrait for TabPrevCommand {
    fn name(&self) -> &'static str {
        "tab_prev"
    }

    fn description(&self) -> &'static str {
        "Switch to previous tab"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Tab(TabAction::Prev))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
