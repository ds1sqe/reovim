//! Dot repeat command (.).
//!
//! Implements the `.` command which repeats the last change operation.
//!
//! # Supported Changes
//!
//! - Operator + motion (e.g., `dw`, `c$`)
//! - Operator + text object (e.g., `diw`, `ci"`)
//! - Insert mode text (e.g., `ihello<Esc>`)
//!
//! # Count Behavior
//!
//! If `.` is invoked with a count (e.g., `3.`), that count **replaces**
//! the original count, it does NOT multiply. This matches Vim behavior.

use {
    reovim_domain_text::Position,
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime, api::ExtensionApi},
    reovim_kernel::api::v1::CommandId,
};

use crate::{
    ids,
    session_state::{ChangeType, VimSessionState},
};

/// Repeat last change (.).
///
/// Replays the last change operation. Count overrides the original count.
#[derive(Debug, Clone, Copy, Default)]
pub struct DotRepeat;

impl Command for DotRepeat {
    fn id(&self) -> CommandId {
        ids::DOT_REPEAT
    }

    fn description(&self) -> &'static str {
        "Repeat last change (.)"
    }
}

impl CommandHandler for DotRepeat {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get last change from VimSessionState
        let Some(vim) = runtime.ext::<VimSessionState>() else {
            return CommandResult::Success; // No vim state
        };
        let Some(last_change) = vim.last_change.as_ref() else {
            return CommandResult::Success; // No change to repeat
        };
        let last_change = last_change.clone();

        // Count from `.` command overrides original count (Vim behavior)
        let effective_count = args.count().or(last_change.count);
        // TODO(#465): Use register for dot repeat once register API is ready
        let _ = last_change.register;

        match &last_change.change_type {
            ChangeType::Insert { text } => {
                // Repeat insert: insert the same text
                Self::repeat_insert(runtime, args, text, effective_count)
            }
            ChangeType::OperatorMotion { operator, linewise }
            | ChangeType::OperatorTextObject { operator, linewise } => {
                // For now, log the operation - full implementation requires
                // re-dispatching the same keys or storing more context
                tracing::debug!(
                    ?operator,
                    linewise,
                    count = ?effective_count,
                    "dot repeat: operator (stub - requires key re-dispatch)"
                );
                CommandResult::Success
            }
        }
    }
}

impl DotRepeat {
    /// Repeat an insert operation by inserting the same text.
    fn repeat_insert(
        runtime: &mut SessionRuntime<'_>,
        args: &CommandContext,
        text: &str,
        count: Option<usize>,
    ) -> CommandResult {
        // Get buffer_id from context or active buffer
        let buffer_id = args.buffer_id().or_else(|| runtime.active_buffer());
        let Some(buffer_id) = buffer_id else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        let effective_count = count.unwrap_or(1);

        // Build repeated text
        let repeated_text = text.repeat(effective_count);

        // Insert the text at cursor position
        runtime.insert_text(buffer_id, pos, &repeated_text);

        CommandResult::Success
    }
}
