//! Mode entry commands.
//!
//! Provides specialized commands for entering insert mode:
//! - `EnterInsertFirstNonBlank` (I)
//! - `EnterInsertEndOfLine` (A)
//! - `OpenLineBelow` (o)
//! - `OpenLineAbove` (O)
//!
//! # Epic #372 - Mode Ownership
//!
//! These commands use `VimMode::INSERT_ID` to transition to insert mode,
//! which is why they belong in the vim module.

use {
    reovim_domain_text::Position,
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::{BufferApi, SessionRuntime, TransitionContext, api::ModeApi},
    reovim_driver_text_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{BufferId, CommandId, OptionScopeId},
};

use {
    crate::{ids, modes::VimMode},
    reovim_module_editor::command::get_line_indent,
};

/// Helper to get cursor position from the active window.
fn get_cursor_position(runtime: &SessionRuntime<'_>) -> Option<Position> {
    let window = runtime.windows().active()?;
    Some(Position::new(window.cursor.line, window.cursor.column))
}

/// Helper to set cursor position on the active window.
#[cfg_attr(coverage_nightly, coverage(off))]
fn set_cursor_position(runtime: &mut SessionRuntime<'_>, pos: Position) {
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor = pos.into();
    }
}

/// Start undo batching for insert mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn begin_insert_batch(runtime: &SessionRuntime<'_>, buffer_id: BufferId) {
    if let Some(pos) = get_cursor_position(runtime)
        && let Some(undo_registry) = runtime.kernel().services.get::<UndoProviderRegistry>()
        && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
    {
        undo_provider.begin_batch(buffer_id, pos);
    }
}

/// Enter insert mode at first non-blank character (I).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertFirstNonBlank;

impl Command for EnterInsertFirstNonBlank {
    fn id(&self) -> CommandId {
        ids::ENTER_INSERT_BOL
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at first non-blank character"
    }
}

impl CommandHandler for EnterInsertFirstNonBlank {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id() {
            if let Some(pos) = get_cursor_position(runtime) {
                // Find first non-blank character on current line
                let first_non_blank = runtime
                    .buffer_line(buffer_id, pos.line)
                    .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));

                set_cursor_position(runtime, Position::new(pos.line, first_non_blank));
            }
            // Start undo batching for insert mode
            begin_insert_batch(runtime, buffer_id);
        }

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter insert mode at end of line (A).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertEndOfLine;

impl Command for EnterInsertEndOfLine {
    fn id(&self) -> CommandId {
        ids::ENTER_INSERT_EOL
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at end of line"
    }
}

impl CommandHandler for EnterInsertEndOfLine {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id() {
            if let Some(pos) = get_cursor_position(runtime) {
                // Move cursor to end of current line
                let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);
                set_cursor_position(runtime, Position::new(pos.line, line_len));
            }
            // Start undo batching for insert mode
            begin_insert_batch(runtime, buffer_id);
        }

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Open line below and enter insert mode (o).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineBelow;

impl Command for OpenLineBelow {
    fn id(&self) -> CommandId {
        ids::OPEN_LINE_BELOW
    }

    fn description(&self) -> &'static str {
        "Open line below and enter insert mode"
    }
}

impl CommandHandler for OpenLineBelow {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Check autoindent option (escape hatch - OptionsApi not yet available)
        let autoindent = runtime
            .kernel()
            .options
            .get("autoindent", OptionScopeId::Buffer(buffer_id))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(pos) = get_cursor_position(runtime) else {
            return CommandResult::error("Failed to get buffer position");
        };

        // Get indent from current line if autoindent is enabled
        let indent = if autoindent {
            runtime
                .buffer_line(buffer_id, pos.line)
                .map(|line| get_line_indent(&line).to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Get end of current line position
        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);
        let insert_pos = Position::new(pos.line, line_len);

        // Insert newline + indent at end of current line
        let insert_text = format!("\n{indent}");
        runtime.insert_text(buffer_id, insert_pos, &insert_text);

        // Position cursor at end of indent on new line
        let indent_len = indent.chars().count();
        set_cursor_position(runtime, Position::new(pos.line + 1, indent_len));

        // Start undo batching for insert mode
        begin_insert_batch(runtime, buffer_id);

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Open line above and enter insert mode (O).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineAbove;

impl Command for OpenLineAbove {
    fn id(&self) -> CommandId {
        ids::OPEN_LINE_ABOVE
    }

    fn description(&self) -> &'static str {
        "Open line above and enter insert mode"
    }
}

impl CommandHandler for OpenLineAbove {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Check autoindent option (escape hatch - OptionsApi not yet available)
        let autoindent = runtime
            .kernel()
            .options
            .get("autoindent", OptionScopeId::Buffer(buffer_id))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(pos) = get_cursor_position(runtime) else {
            return CommandResult::error("Failed to get buffer position");
        };

        // Get indent from current line if autoindent is enabled
        let indent = if autoindent {
            runtime
                .buffer_line(buffer_id, pos.line)
                .map(|line| get_line_indent(&line).to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Insert indent + newline at start of current line
        let insert_pos = Position::new(pos.line, 0);
        let insert_text = format!("{indent}\n");
        runtime.insert_text(buffer_id, insert_pos, &insert_text);

        // Position cursor at end of indent on the new line (which is now at pos.line)
        let indent_len = indent.chars().count();
        set_cursor_position(runtime, Position::new(pos.line, indent_len));

        // Start undo batching for insert mode
        begin_insert_batch(runtime, buffer_id);

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::uninlined_format_args, clippy::significant_drop_tightening)]
#[path = "tests/mode_entry.rs"]
mod tests;
