//! Completion-related commands (unified command-event types)

use reovim_core::{
    command::traits::*,
    declare_event_command,
    event_bus::{DynEvent, Event},
};

// === Trigger Command (custom impl for context data + priority) ===

/// Trigger completion at current cursor position
#[derive(Debug, Clone, Copy)]
pub struct CompletionTrigger {
    /// Buffer ID where completion was triggered
    pub buffer_id: usize,
}

impl CompletionTrigger {
    /// Create instance from buffer ID
    #[must_use]
    pub const fn new(buffer_id: usize) -> Self {
        Self { buffer_id }
    }
}

impl CommandTrait for CompletionTrigger {
    fn name(&self) -> &'static str {
        "completion_trigger"
    }

    fn description(&self) -> &'static str {
        "Trigger completion at cursor position"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let event = Self::new(ctx.buffer_id);
        CommandResult::EmitEvent(DynEvent::new(event))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Event for CompletionTrigger {
    fn priority(&self) -> u32 {
        50 // High priority for UI updates
    }
}

// === Navigation & Action Commands (using macro) ===

declare_event_command! {
    CompletionSelectNext,
    id: "completion_next",
    description: "Select next completion item",
}

declare_event_command! {
    CompletionSelectPrev,
    id: "completion_prev",
    description: "Select previous completion item",
}

declare_event_command! {
    CompletionConfirm,
    id: "completion_confirm",
    description: "Confirm and insert selected completion",
}

declare_event_command! {
    CompletionDismiss,
    id: "completion_dismiss",
    description: "Dismiss completion popup",
}
