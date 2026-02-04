//! Command-line completion commands

use reovim_core::{
    command::{
        CommandResult, CommandTrait, ExecutionContext,
        traits::{CommandLineAction, DeferredAction},
    },
    declare_event_command,
    event_bus::{Event, priority},
};

/// Trigger command-line completion (Tab key)
///
/// This command defers to the runtime to get the command line context,
/// which then emits `CmdlineCompletionTriggered` with the context.
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineComplete;

impl CommandTrait for CmdlineComplete {
    fn name(&self) -> &'static str {
        "cmdline_complete"
    }

    fn description(&self) -> &'static str {
        "Trigger command-line completion (Tab)"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        // Defer to runtime to get command line context and emit CmdlineCompletionTriggered
        CommandResult::DeferToRuntime(DeferredAction::CommandLine(CommandLineAction::Complete))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Event for CmdlineComplete {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// Select previous completion item (Shift-Tab)
///
/// This command defers to the runtime to emit CmdlineCompletionPrevRequested.
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineCompletePrev;

impl CommandTrait for CmdlineCompletePrev {
    fn name(&self) -> &'static str {
        "cmdline_complete_prev"
    }

    fn description(&self) -> &'static str {
        "Select previous completion item"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        // Defer to runtime to emit CmdlineCompletionPrevRequested
        CommandResult::DeferToRuntime(DeferredAction::CommandLine(CommandLineAction::CompletePrev))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Event for CmdlineCompletePrev {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// These commands use the unified event-command pattern since they don't need
// runtime context - they're handled purely by the plugin.

declare_event_command! {
    CmdlineCompleteNext,
    id: "cmdline_complete_next",
    description: "Select next completion item",
}

declare_event_command! {
    CmdlineCompleteConfirm,
    id: "cmdline_complete_confirm",
    description: "Confirm and insert selected completion",
}

declare_event_command! {
    CmdlineCompleteDismiss,
    id: "cmdline_complete_dismiss",
    description: "Dismiss completion popup",
}
