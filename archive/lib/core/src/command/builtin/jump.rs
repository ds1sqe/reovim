//! Jump list navigation commands (Ctrl-O / Ctrl-I)

use {
    crate::command::traits::{CommandResult, CommandTrait, DeferredAction, ExecutionContext},
    std::any::Any,
};

/// Jump to older position (Ctrl-O)
#[derive(Debug, Clone)]
pub struct JumpOlderCommand;

impl CommandTrait for JumpOlderCommand {
    fn name(&self) -> &'static str {
        "jump_older"
    }

    fn description(&self) -> &'static str {
        "Jump to older cursor position"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::JumpOlder)
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Jump to newer position (Ctrl-I)
#[derive(Debug, Clone)]
pub struct JumpNewerCommand;

impl CommandTrait for JumpNewerCommand {
    fn name(&self) -> &'static str {
        "jump_newer"
    }

    fn description(&self) -> &'static str {
        "Jump to newer cursor position"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::JumpNewer)
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
