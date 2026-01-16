//! Mode switching commands.
//!
//! Provides commands for switching between editor modes:
//! - Enter insert mode (i, a)
//! - Exit to normal mode (Escape)
//! - Enter window mode (Ctrl-W)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult, ModeAction},
    reovim_kernel::api::v1::{CommandId, KernelContext, Position, events::ModeChanged},
};

use super::super::mode::{EDITOR_MODULE, EditorMode};

/// Enter insert mode (before cursor).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertMode;

impl Command for EnterInsertMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode"
    }
}

impl CommandHandler for EnterInsertMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        // Note: The actual mode stack change happens in the runner via a callback
        // or by the caller checking the result. For now, we just emit the event.

        CommandResult::Success
    }
}

/// Enter insert mode after cursor (a).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertModeAppend;

impl Command for EnterInsertModeAppend {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert-after")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode after cursor (append)"
    }
}

impl CommandHandler for EnterInsertModeAppend {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Move cursor right first, then enter insert mode
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            // Move right only if not at end of line
            if pos.column < line_len {
                buffer.set_position(Position::new(pos.line, pos.column + 1));
            }
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Exit to normal mode (Escape from insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitToNormal;

impl Command for ExitToNormal {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "exit-insert")
    }

    fn description(&self) -> &'static str {
        "Exit insert mode and return to normal mode"
    }
}

impl CommandHandler for ExitToNormal {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Move cursor left one position when exiting insert mode (Vim behavior)
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            if pos.column > 0 {
                buffer.set_position(Position::new(pos.line, pos.column - 1));
            }
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("insert", EditorMode::NORMAL_ID));

        CommandResult::Success
    }
}

/// Enter window management mode (Ctrl-W in normal mode).
///
/// This pushes "window" mode onto the mode stack. In window mode,
/// subsequent keys (h/j/k/l for navigation, s/v for splits, etc.)
/// are handled by the layout module's keybindings.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterWindowMode;

impl Command for EnterWindowMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-window-mode")
    }

    fn description(&self) -> &'static str {
        "Enter window management mode"
    }
}

impl CommandHandler for EnterWindowMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus.emit(ModeChanged::new("normal", "window"));

        // Return mode action to push window mode onto stack
        CommandResult::ModeAction(ModeAction::Push("window".to_string()))
    }
}
