//! Mode switching commands.
//!
//! Provides commands for switching between Vim modes:
//! - Enter insert mode (i, a)
//! - Exit to normal mode (Escape)
//! - Enter window mode (Ctrl-W)
//! - Enter/exit command-line mode
//! - Exit operator-pending mode
//!
//! # Epic #372 - Mode Ownership
//!
//! These commands use `VimMode::*_ID` constants directly, which is why they
//! belong in the vim module rather than the generic editor module.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult, ModeAction},
    reovim_kernel::api::v1::{CommandId, KernelContext, Position, events::ModeChanged},
};

use crate::modes::{VIM_MODULE, VimMode};

/// Enter insert mode (before cursor).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertMode;

impl Command for EnterInsertMode {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "enter-insert")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode"
    }
}

impl CommandHandler for EnterInsertMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Enter insert mode after cursor (a).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertModeAppend;

impl Command for EnterInsertModeAppend {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "enter-insert-after")
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
            .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Exit to normal mode (Escape from insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitToNormal;

impl Command for ExitToNormal {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "exit-insert")
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
            .emit(ModeChanged::with_mode_id("insert", VimMode::NORMAL_ID));

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
        CommandId::new(VIM_MODULE, "enter-window-mode")
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

/// Exit operator-pending mode (Escape cancels the operator).
///
/// This command is called when the user presses Escape while in operator-pending
/// mode. It clears the pending operator state and returns to normal mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitOperatorPending;

impl Command for ExitOperatorPending {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "exit-operator-pending")
    }

    fn description(&self) -> &'static str {
        "Cancel pending operator and return to normal mode"
    }
}

impl CommandHandler for ExitOperatorPending {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("operator-pending", VimMode::NORMAL_ID));

        // Return mode action to pop operator-pending mode off the stack
        // The event loop will clear the pending operator state
        CommandResult::ModeAction(ModeAction::Pop)
    }
}

/// Enter command-line mode (`:` in normal mode).
///
/// This pushes "commandline" mode onto the mode stack. In command-line mode,
/// the user can type Ex commands like `:w`, `:q`, `:set`, etc.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterCommandLineMode;

impl Command for EnterCommandLineMode {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "enter-commandline")
    }

    fn description(&self) -> &'static str {
        "Enter command-line mode"
    }
}

impl CommandHandler for EnterCommandLineMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", VimMode::COMMANDLINE_ID));

        // Return mode action to transition to commandline mode
        CommandResult::ModeAction(ModeAction::Set(VimMode::COMMANDLINE_ID.to_string()))
    }
}

/// Exit command-line mode (Escape or Enter).
///
/// Returns to normal mode from command-line mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitCommandLineMode;

impl Command for ExitCommandLineMode {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "exit-commandline")
    }

    fn description(&self) -> &'static str {
        "Exit command-line mode and return to normal mode"
    }
}

impl CommandHandler for ExitCommandLineMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("commandline", VimMode::NORMAL_ID));

        CommandResult::ModeAction(ModeAction::Set(VimMode::NORMAL_ID.to_string()))
    }
}
