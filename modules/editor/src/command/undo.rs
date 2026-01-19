//! Undo/Redo commands.
//!
//! Provides commands for undo/redo operations:
//! - `UndoCommand` (u)
//! - `RedoCommand` (Ctrl-R)
//!
//! # Architecture (Epic #385)
//!
//! These commands are stubs that will be implemented when `SessionContext`
//! provides direct access to undo state.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext},
};

use crate::ids;

/// Undo the last change.
///
/// Note: Will be implemented via `SessionContext` when available.
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // TODO: Implement via SessionContext when available
        CommandResult::Success
    }
}

/// Redo the last undone change.
///
/// Note: Will be implemented via `SessionContext` when available.
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // TODO: Implement via SessionContext when available
        CommandResult::Success
    }
}
