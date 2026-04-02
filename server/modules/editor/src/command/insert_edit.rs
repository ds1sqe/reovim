//! Insert mode edit commands.
//!
//! Provides commands for editing in insert mode:
//! - `InsertNewline` (Enter)
//! - `InsertTab` (Tab)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, OptionScopeId},
    reovim_types_text::Position,
};

use crate::ids;

/// Extract the leading whitespace (indent) from a line.
///
/// Returns a string slice containing only the leading whitespace characters.
/// This preserves the exact mix of tabs and spaces.
#[must_use]
pub fn get_line_indent(line: &str) -> &str {
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Get indent from current line if autoindent is enabled
        let indent = if autoindent {
            runtime
                .buffer_line(buffer_id, pos.line)
                .map(|line| get_line_indent(&line).to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Insert newline + indent (splits the line at cursor position)
        let insert_text = format!("\n{indent}");
        runtime.insert_text(buffer_id, pos, &insert_text);

        // Update cursor position to end of indent on new line
        let indent_len = indent.chars().count();
        let new_pos = Position::new(pos.line + 1, indent_len);
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get options (escape hatch - OptionsApi not yet available)
        let scope = OptionScopeId::Buffer(buffer_id);
        let expandtab = runtime
            .kernel()
            .options
            .get("expandtab", scope)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let tabstop = runtime
            .kernel()
            .options
            .get("tabstop", scope)
            .and_then(|v| v.as_int())
            .map_or(4, |n| n.max(1) as usize);

        let text = if expandtab {
            " ".repeat(tabstop)
        } else {
            "\t".to_string()
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Insert tab/spaces at current position
        runtime.insert_text(buffer_id, pos, &text);

        // Update cursor position to after inserted text
        let text_len = text.chars().count();
        let new_pos = Position::new(pos.line, pos.column + text_len);
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "tests/insert_edit.rs"]
mod tests;
