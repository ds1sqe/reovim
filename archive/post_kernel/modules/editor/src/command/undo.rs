//! Undo/Redo commands.
//!
//! Provides commands for undo/redo operations:
//! - `UndoCommand` (u)
//! - `RedoCommand` (Ctrl-R)
//!
//! # Architecture
//!
//! These commands use the `UndoApi` trait to perform undo/redo operations.
//! The actual undo tree is managed by the undo module's `UndoRegistry`.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        SessionRuntime,
        api::{BufferApi, UndoApi},
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

/// Undo the last change.
///
/// Uses `UndoApi` to traverse the undo tree and apply inverse edits.
/// Supports count argument to undo multiple changes at once.
#[derive(Debug, Clone, Copy, Default)]
pub struct UndoCommand;

impl Command for UndoCommand {
    fn id(&self) -> CommandId {
        ids::UNDO
    }

    fn description(&self) -> &'static str {
        "Undo the last change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to undo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["u", "undo"]
    }
}

impl CommandHandler for UndoCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer) = runtime.active_buffer() else {
            return CommandResult::Error("No active buffer".to_string());
        };

        let count = args.count().unwrap_or(1);
        let mut undone = 0;

        for _ in 0..count {
            if runtime.undo(buffer).is_some() {
                undone += 1;
            } else {
                break; // Nothing left to undo
            }
        }

        if undone == 0 {
            CommandResult::Error("Already at oldest change".to_string())
        } else {
            CommandResult::Success
        }
    }
}

/// Redo the last undone change.
///
/// Uses `UndoApi` to traverse the undo tree and apply edits.
/// Supports count argument to redo multiple changes at once.
#[derive(Debug, Clone, Copy, Default)]
pub struct RedoCommand;

impl Command for RedoCommand {
    fn id(&self) -> CommandId {
        ids::REDO
    }

    fn description(&self) -> &'static str {
        "Redo the last undone change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to redo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["redo"]
    }
}

impl CommandHandler for RedoCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer) = runtime.active_buffer() else {
            return CommandResult::Error("No active buffer".to_string());
        };

        let count = args.count().unwrap_or(1);
        let mut redone = 0;

        for _ in 0..count {
            if runtime.redo(buffer).is_some() {
                redone += 1;
            } else {
                break; // Nothing left to redo
            }
        }

        if redone == 0 {
            CommandResult::Error("Already at newest change".to_string())
        } else {
            CommandResult::Success
        }
    }
}
