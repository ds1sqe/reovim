//! Command execution abstraction.
//!
//! This module provides traits for command execution:
//! - [`CommandApi`] - For resolvers to execute commands
//! - [`CommandHandle`] - Opaque handle for re-entrant command execution
//! - [`CommandExecutor`] - Abstraction over runner's `CommandRegistry`
//!
//! # Design
//!
//! Following the mechanism vs policy principle:
//! - **Session driver defines traits** (this module)
//! - **Runner implements `CommandExecutor`** (via `CommandRegistry`)
//! - **Modules use `CommandApi`** to execute commands
//!
//! This keeps the session driver decoupled from runner types.
//!
//! # Re-Entrant Execution (#547)
//!
//! `CommandExecutor::get_handle()` returns an owned `Arc<dyn CommandHandle>`,
//! releasing the borrow on `self.executor`. Then `handle.execute(self, &ctx)`
//! can pass `&mut self` cleanly. This enables commands to call other commands
//! via `runtime.execute_command()`.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::CommandApi;
//!
//! fn execute_motion<S: CommandApi>(session: &mut S, motion_cmd: CommandId) {
//!     let ctx = CommandContext::new();
//!     let result = session.execute_command(motion_cmd, ctx);
//!     // Handle result...
//! }
//! ```

use {
    reovim_kernel::api::v1::CommandId,
    reovim_subsys_command_types::{CommandContext, CommandResult},
    std::sync::Arc,
};

/// Command execution for resolvers.
///
/// Provides a way for resolvers to execute commands without
/// knowing about the runner's command registry.
pub trait CommandApi: Send {
    /// Execute a command directly.
    ///
    /// The command is looked up and executed with the given context.
    fn execute_command(&mut self, cmd: CommandId, ctx: CommandContext) -> CommandResult;
}

/// Opaque handle for executing a command with a `SessionRuntime`.
///
/// This trait bridges `CommandHandler` (in `reovim-driver-command`) and
/// `SessionRuntime` (in this crate) without creating a dependency cycle.
/// The server creates these by wrapping `Arc<dyn CommandHandler>` in a
/// `HandlerBridge` wrapper (#547).
pub trait CommandHandle: Send + Sync {
    /// Execute the command with the given runtime and arguments.
    fn execute(
        &self,
        runtime: &mut crate::SessionRuntime<'_>,
        ctx: &CommandContext,
    ) -> CommandResult;
}

/// Trait for command handler lookup.
///
/// Runner's `CommandRegistry` implements this trait.
/// This abstraction prevents session driver from depending on runner types.
///
/// # Design
///
/// Instead of `SessionRuntime` holding a concrete `CommandRegistry`,
/// it holds `&dyn CommandExecutor`. This maintains proper dependency
/// direction: runner depends on session driver, not vice versa.
///
/// # Re-Entrant Execution (#547)
///
/// `get_handle()` returns an owned `Arc<dyn CommandHandle>`, releasing
/// the borrow on the executor. The caller then invokes
/// `handle.execute(runtime, ctx)` with full `&mut SessionRuntime` access.
pub trait CommandExecutor: Send + Sync {
    /// Look up a command handler by ID.
    ///
    /// Returns an `Arc` so the caller can execute the handler
    /// through its own runtime (re-entrant execution).
    /// Returns `None` if the command is not found.
    fn get_handle(&self, id: &CommandId) -> Option<Arc<dyn CommandHandle>>;
}
#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
