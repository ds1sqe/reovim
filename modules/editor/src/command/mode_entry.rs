//! Mode entry commands (Phase 2).
//!
//! Provides specialized commands for entering insert mode:
//! - `EnterInsertFirstNonBlank` (I)
//! - `EnterInsertEndOfLine` (A)
//! - `OpenLineBelow` (o)
//! - `OpenLineAbove` (O)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_kernel::api::v1::{CommandId, KernelContext, Position, events::ModeChanged},
};

use super::super::mode::{EDITOR_MODULE, EditorMode};

/// Enter insert mode at first non-blank character (I).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertFirstNonBlank;

impl Command for EnterInsertFirstNonBlank {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert-bol")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at first non-blank character"
    }
}

impl CommandHandler for EnterInsertFirstNonBlank {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();

            // Find first non-blank character on current line
            let first_non_blank = buffer
                .line(pos.line)
                .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));

            buffer.set_position(Position::new(pos.line, first_non_blank));
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Enter insert mode at end of line (A).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertEndOfLine;

impl Command for EnterInsertEndOfLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert-eol")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at end of line"
    }
}

impl CommandHandler for EnterInsertEndOfLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();

            // Move cursor to end of current line
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            buffer.set_position(Position::new(pos.line, line_len));
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Open line below and enter insert mode (o).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineBelow;

impl Command for OpenLineBelow {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "open-line-below")
    }

    fn description(&self) -> &'static str {
        "Open line below and enter insert mode"
    }
}

impl CommandHandler for OpenLineBelow {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();

        // Move to end of current line
        let line_len = buffer.line_len(pos.line).unwrap_or(0);
        buffer.set_position(Position::new(pos.line, line_len));

        // Capture cursor before insert (after move to end of line)
        let cursor_before = buffer.position();
        // Insert newline (creates new line below)
        let edit = buffer.insert("\n");
        // Cursor is now at start of new line
        let cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Open line above and enter insert mode (O).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineAbove;

impl Command for OpenLineAbove {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "open-line-above")
    }

    fn description(&self) -> &'static str {
        "Open line above and enter insert mode"
    }
}

impl CommandHandler for OpenLineAbove {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();

        // Move to start of current line
        buffer.set_position(Position::new(pos.line, 0));

        // Capture cursor before insert
        let cursor_before = buffer.position();
        // Insert newline before current line content
        let edit = buffer.insert("\n");

        // Move cursor up to the new empty line
        buffer.set_position(Position::new(pos.line, 0));
        let cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}
