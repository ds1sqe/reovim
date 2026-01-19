//! Insert mode edit commands.
//!
//! Provides commands for editing in insert mode:
//! - `InsertNewline` (Enter)
//! - `InsertTab` (Tab)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_kernel::api::v1::{CommandId, KernelContext, OptionScopeId},
};

use crate::ids;

/// Extract the leading whitespace (indent) from a line.
///
/// Returns a string slice containing only the leading whitespace characters.
/// This preserves the exact mix of tabs and spaces.
#[must_use]
fn get_line_indent(line: &str) -> &str {
    let non_ws_pos = line
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map_or(line.len(), |(i, _)| i);
    &line[..non_ws_pos]
}

/// Insert a newline at cursor position (Enter in insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertNewline;

impl Command for InsertNewline {
    fn id(&self) -> CommandId {
        ids::INSERT_NEWLINE
    }

    fn description(&self) -> &'static str {
        "Insert newline at cursor position"
    }
}

impl CommandHandler for InsertNewline {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Check autoindent option
        let autoindent = ctx
            .options
            .get("autoindent", OptionScopeId::Buffer(buffer_id))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();

        // Get indent from current line if autoindent is enabled
        let indent = if autoindent {
            buffer
                .line(pos.line)
                .map(|line| get_line_indent(line).to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };

        let cursor_before = buffer.position();

        // Insert newline + indent (splits the line at cursor position)
        let insert_text = format!("\n{indent}");
        let edit = buffer.insert(&insert_text);

        // Cursor is now at end of indent on new line
        let _cursor_after = buffer.position();
        drop(buffer);

        // TODO: Return edit action via different mechanism when SessionContext is available
        let _ = (buffer_id, edit, cursor_before); // Suppress unused warnings
        CommandResult::Success
    }
}

/// Insert a tab at cursor position (Tab in insert mode).
///
/// Respects `expandtab` and `tabstop` options.
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertTab;

impl Command for InsertTab {
    fn id(&self) -> CommandId {
        ids::INSERT_TAB
    }

    fn description(&self) -> &'static str {
        "Insert tab at cursor position"
    }
}

impl CommandHandler for InsertTab {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Get options (with defaults). Use buffer-local scope if available.
        let scope = OptionScopeId::Buffer(buffer_id);
        let expandtab = ctx
            .options
            .get("expandtab", scope)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let tabstop = ctx
            .options
            .get("tabstop", scope)
            .and_then(|v| v.as_int())
            .map_or(4, |n| n.max(1) as usize);

        let text = if expandtab {
            " ".repeat(tabstop)
        } else {
            "\t".to_string()
        };

        let mut buffer = buffer_arc.write();
        let _cursor_before = buffer.position();
        let _edit = buffer.insert(&text);
        let _cursor_after = buffer.position();
        drop(buffer);

        // TODO: Return edit action via different mechanism when SessionContext is available
        CommandResult::Success
    }
}
