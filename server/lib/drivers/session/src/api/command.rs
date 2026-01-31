//! Command execution abstraction.
//!
//! This module provides traits for command execution:
//! - [`CommandApi`] - For resolvers to execute commands
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
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_kernel::api::v1::{CommandId, KernelContext},
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

/// Trait for command lookup and execution.
///
/// Runner's `CommandRegistry` implements this trait.
/// This abstraction prevents session driver from depending on runner types.
///
/// # Design
///
/// Instead of `SessionRuntime` holding a concrete `CommandRegistry`,
/// it holds `&dyn CommandExecutor`. This maintains proper dependency
/// direction: runner depends on session driver, not vice versa.
pub trait CommandExecutor: Send + Sync {
    /// Execute a command by ID.
    ///
    /// Returns `None` if the command is not found.
    ///
    /// # Note
    ///
    /// `KernelContext` uses interior mutability (`Arc<RwLock<...>>`) for all
    /// mutable state, so `&KernelContext` is sufficient for command execution.
    fn execute(
        &self,
        cmd: &CommandId,
        ctx: &CommandContext,
        kernel: &KernelContext,
    ) -> Option<CommandResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify trait object safety
    #[test]
    fn test_command_executor_object_safe() {
        fn _accepts_ref(_: &dyn CommandExecutor) {}
        fn _accepts_box(_: Box<dyn CommandExecutor>) {}
    }
}
