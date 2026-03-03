//! Command handlers for the completion popup.
//!
//! Commands for triggering, navigating, confirming, and dismissing completions.
//! These handlers operate on `CompletionState` stored in the session's
//! `ExtensionMap`.

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_completion::{CompletionContext, CompletionSourceRegistry},
    reovim_driver_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{
    ids,
    state::{CompletionItemSnapshot, CompletionState},
};

// ============================================================================
// Trigger completion
// ============================================================================

/// Trigger the completion popup with items from all registered sources.
#[derive(Debug, Clone, Copy, Default)]
pub struct Trigger;

impl reovim_driver_command::Command for Trigger {
    fn id(&self) -> CommandId {
        ids::TRIGGER
    }

    fn description(&self) -> &'static str {
        "Trigger completion"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Trigger {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Build completion context from current buffer state.
        let Some(ctx) = build_context(runtime) else {
            return CommandResult::Success;
        };

        // Gather items from all registered sources.
        let registry = runtime.kernel().services.get::<CompletionSourceRegistry>();
        let mut all_items = Vec::new();

        if let Some(registry) = registry {
            let mut sources = registry.all();
            // Sort by priority (highest first).
            sources.sort_by_key(|s| std::cmp::Reverse(s.priority()));

            for source in &sources {
                if source.is_available(&ctx) {
                    let items = source.complete(&ctx);
                    all_items.extend(items);
                }
            }
        }

        if all_items.is_empty() {
            return CommandResult::Success;
        }

        let snapshots: Vec<CompletionItemSnapshot> = all_items
            .iter()
            .map(CompletionItemSnapshot::from_item)
            .collect();

        let state = runtime.ext_mut::<CompletionState>();
        state.open(snapshots, &ctx.prefix);

        CommandResult::Success
    }
}

// ============================================================================
// Navigation
// ============================================================================

/// Move to the next completion item.
#[derive(Debug, Clone, Copy, Default)]
pub struct Next;

impl reovim_driver_command::Command for Next {
    fn id(&self) -> CommandId {
        ids::NEXT
    }

    fn description(&self) -> &'static str {
        "Next completion item"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Next {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<CompletionState>();
        if state.active {
            state.next();
        }
        CommandResult::Success
    }
}

/// Move to the previous completion item.
#[derive(Debug, Clone, Copy, Default)]
pub struct Prev;

impl reovim_driver_command::Command for Prev {
    fn id(&self) -> CommandId {
        ids::PREV
    }

    fn description(&self) -> &'static str {
        "Previous completion item"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Prev {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<CompletionState>();
        if state.active {
            state.prev();
        }
        CommandResult::Success
    }
}

// ============================================================================
// Confirm / Dismiss
// ============================================================================

/// Confirm the selected completion item.
#[derive(Debug, Clone, Copy, Default)]
pub struct Confirm;

impl reovim_driver_command::Command for Confirm {
    fn id(&self) -> CommandId {
        ids::CONFIRM
    }

    fn description(&self) -> &'static str {
        "Confirm selected completion"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Confirm {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<CompletionState>();
        // TODO(#521): Insert the selected item's text into the buffer.
        // For now, just close the popup.
        state.close();
        CommandResult::Success
    }
}

/// Dismiss the completion popup.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dismiss;

impl reovim_driver_command::Command for Dismiss {
    fn id(&self) -> CommandId {
        ids::DISMISS
    }

    fn description(&self) -> &'static str {
        "Dismiss completion popup"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Dismiss {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<CompletionState>();
        state.close();
        CommandResult::Success
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Build a `CompletionContext` from the current buffer state.
///
/// Returns `None` if there is no active buffer or window.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_context(runtime: &SessionRuntime<'_>) -> Option<CompletionContext> {
    let buffer_id = runtime.active_buffer()?;
    let content = runtime.buffer_content(buffer_id)?;

    // Get cursor from per-client window (#471).
    let window = runtime.windows().active()?;
    let cursor = Position::new(window.cursor.line, window.cursor.column);
    let cursor_line = cursor.line;
    let cursor_col = cursor.column;

    // Extract the prefix: the word fragment before the cursor on the current line.
    let prefix = {
        let line_text = content.lines().nth(cursor_line).unwrap_or("");
        let before_cursor = if cursor_col <= line_text.len() {
            &line_text[..cursor_col]
        } else {
            line_text
        };
        before_cursor
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map_or(before_cursor, |pos| &before_cursor[pos + 1..])
            .to_owned()
    };

    // Calculate byte offset of cursor.
    let cursor_offset = content
        .lines()
        .take(cursor_line)
        .map(|l| l.len() + 1) // +1 for newline
        .sum::<usize>()
        + cursor_col;

    let file_path = runtime.buffer_file_path(buffer_id);

    Some(CompletionContext {
        content,
        cursor_offset,
        line: cursor_line,
        col: cursor_col,
        prefix,
        buffer_id: buffer_id.as_usize(),
        file_path,
        language_id: None,
    })
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(Trigger),
        Box::new(Next),
        Box::new(Prev),
        Box::new(Confirm),
        Box::new(Dismiss),
    ]
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[test]
    fn trigger_metadata() {
        let cmd = Trigger;
        assert_eq!(cmd.id(), ids::TRIGGER);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn next_metadata() {
        let cmd = Next;
        assert_eq!(cmd.id(), ids::NEXT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn prev_metadata() {
        let cmd = Prev;
        assert_eq!(cmd.id(), ids::PREV);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn confirm_metadata() {
        let cmd = Confirm;
        assert_eq!(cmd.id(), ids::CONFIRM);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn dismiss_metadata() {
        let cmd = Dismiss;
        assert_eq!(cmd.id(), ids::DISMISS);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_count() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 5);
    }

    #[test]
    fn command_handlers_unique_ids() {
        let handlers = command_handlers();
        let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
        let mut deduped = ids.clone();
        deduped.sort_by_key(CommandId::name);
        deduped.dedup_by_key(|id| id.name());
        assert_eq!(ids.len(), deduped.len());
    }
}
