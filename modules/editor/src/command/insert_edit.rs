//! Insert mode edit commands.
//!
//! Provides commands for editing in insert mode:
//! - `InsertNewline` (Enter)
//! - `InsertTab` (Tab)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_kernel::api::v1::{CommandId, KernelContext, OptionScopeId},
};

use super::super::mode::EDITOR_MODULE;

/// Insert a newline at cursor position (Enter in insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertNewline;

impl Command for InsertNewline {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "insert-newline")
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

        let mut buffer = buffer_arc.write();
        let cursor_before = buffer.position();
        // Insert newline splits the line at cursor position
        let edit = buffer.insert("\n");
        let cursor_after = buffer.position();
        drop(buffer);

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Insert a tab at cursor position (Tab in insert mode).
///
/// Respects `expandtab` and `tabstop` options.
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertTab;

impl Command for InsertTab {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "insert-tab")
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
        let cursor_before = buffer.position();
        let edit = buffer.insert(&text);
        let cursor_after = buffer.position();
        drop(buffer);

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}
