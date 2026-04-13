//! Command execution trait for the command driver.
//!
//! The metadata traits ([`Command`], [`CommandPriority`]) live in
//! [`reovim_subsys_command`]. This module defines the execution trait
//! [`CommandHandler`] which requires domain-specific types.

use {
    reovim_driver_session::SessionRuntime,
    reovim_subsys_command::{Command, CommandContext, CommandResult},
};

/// Command execution trait.
///
/// Commands that can be executed implement this trait in addition to [`Command`].
///
/// # Design Philosophy
///
/// Separating execution from metadata allows:
/// - Querying command info without execution capability
/// - Different execution strategies (sync, async, background)
/// - Testing command metadata independently
///
/// # Session Runtime
///
/// Commands receive a [`SessionRuntime`] which provides access to:
/// - **`ModeApi`** - Mode stack operations (push, pop, set)
/// - **`BufferApi`** - Buffer content and cursor operations
/// - **`WindowApi`** - Window management and focus
/// - **`ExtensionApi`** - Per-session module state
/// - **`ChangeTracker`** - State change accumulation
///
/// For operations not yet covered by these APIs, use the escape hatch:
/// ```ignore
/// let kernel = runtime.kernel();
/// let buffer = kernel.buffers.get(buffer_id)?;
/// ```
///
/// # Example
///
/// ```ignore
/// use reovim_driver_command::{Command, CommandHandler, CommandContext, CommandResult};
/// use reovim_driver_session::{SessionRuntime, ModeApi, TransitionContext};
/// use reovim_kernel::api::v1::{CommandId, ModuleId};
///
/// struct EnterInsertMode;
///
/// impl Command for EnterInsertMode {
///     fn id(&self) -> CommandId {
///         CommandId::new(ModuleId::new("vim"), "enter-insert-mode")
///     }
///     fn description(&self) -> &'static str { "Enter insert mode" }
/// }
///
/// impl CommandHandler for EnterInsertMode {
///     fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
///         let insert_mode = ModeId::new(ModuleId::new("vim"), "insert");
///         runtime.set_mode(insert_mode, TransitionContext::new());
///         CommandResult::Success
///     }
/// }
/// ```
pub trait CommandHandler: Command {
    /// Execute the command.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Session runtime providing API trait access and kernel escape hatch
    /// * `args` - The command arguments parsed from user input
    ///
    /// # Returns
    ///
    /// A [`CommandResult`] indicating success, error, or special results like quit.
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult;
}

#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
