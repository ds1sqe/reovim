//! Completion-related commands (unified command-event types)

use reovim_core::{
    buffer::TextOps,
    command::traits::*,
    declare_event_command,
    event_bus::{DynEvent, Event},
};

use crate::CompletionRequest;

// === Trigger Command (custom impl for context data + priority) ===

/// Trigger completion at current cursor position (command)
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
        // Extract buffer content and cursor position
        let content = ctx.buffer.content_to_string();
        let cursor_row = ctx.buffer.cur.y as usize;
        let cursor_col = ctx.buffer.cur.x as usize;

        // Get current line and extract word prefix
        let line = ctx
            .buffer
            .contents
            .get(cursor_row)
            .map(|l| l.inner.clone())
            .unwrap_or_default();

        // Find word start by scanning backwards from cursor
        let line_chars: Vec<char> = line.chars().collect();
        let mut word_start_col = cursor_col;
        while word_start_col > 0 {
            let ch = line_chars.get(word_start_col - 1).copied().unwrap_or(' ');
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            word_start_col -= 1;
        }

        // Extract prefix (text already typed)
        let prefix: String = line_chars[word_start_col..cursor_col].iter().collect();

        // Create completion request event
        let request = CompletionRequest {
            buffer_id: ctx.buffer_id,
            file_path: ctx.buffer.file_path.clone(),
            content,
            cursor_row: cursor_row as u32,
            cursor_col: cursor_col as u32,
            line,
            prefix,
            word_start_col: word_start_col as u32,
            trigger_char: None,
        };

        // Emit CompletionTriggered event with full context
        CommandResult::EmitEvent(DynEvent::new(CompletionTriggered { request }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Event emitted when completion is triggered (contains full context)
#[derive(Debug, Clone)]
pub struct CompletionTriggered {
    /// The completion request with all context
    pub request: CompletionRequest,
}

impl Event for CompletionTriggered {
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
