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
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_kernel::api::v1::{CommandId, KernelContext, Position, events::ModeChanged},
};

use crate::{ids, modes::VimMode};

/// Enter insert mode (before cursor).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertMode;

impl Command for EnterInsertMode {
    fn id(&self) -> CommandId {
        ids::ENTER_INSERT
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
        ids::ENTER_INSERT_AFTER
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
        ids::EXIT_INSERT
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
        ids::ENTER_WINDOW_MODE
    }

    fn description(&self) -> &'static str {
        "Enter window management mode"
    }
}

impl CommandHandler for EnterWindowMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus.emit(ModeChanged::new("normal", "window"));

        // TODO: Mode stack handling via SessionContext when available
        CommandResult::Success
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
        ids::EXIT_OPERATOR_PENDING
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

        // TODO: Mode stack handling via SessionContext when available
        CommandResult::Success
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
        ids::ENTER_COMMANDLINE
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

        // TODO: Mode stack handling via SessionContext when available
        CommandResult::Success
    }
}

/// Exit command-line mode (Escape or Enter).
///
/// Returns to normal mode from command-line mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitCommandLineMode;

impl Command for ExitCommandLineMode {
    fn id(&self) -> CommandId {
        ids::EXIT_COMMANDLINE
    }

    fn description(&self) -> &'static str {
        "Exit command-line mode and return to normal mode"
    }
}

impl CommandHandler for ExitCommandLineMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("commandline", VimMode::NORMAL_ID));

        // TODO: Mode stack handling via SessionContext when available
        CommandResult::Success
    }
}
