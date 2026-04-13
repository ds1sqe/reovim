#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Command driver for reovim - command execution framework.
//!
//! Core command metadata traits are in [`reovim_subsys_command`]. This crate
//! provides the execution trait [`CommandHandler`] which requires domain-specific
//! types ([`SessionRuntime`]).
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_command::{Command, CommandHandler, CommandContext, CommandResult, ArgSpec, ArgKind};
//! use reovim_driver_session::SessionRuntime;
//! use reovim_kernel::api::v1::{CommandId, ModuleId};
//!
//! const MY_MODULE: ModuleId = ModuleId::new_const("my-module");
//!
//! pub struct CursorDown;
//!
//! impl Command for CursorDown {
//!     fn id(&self) -> CommandId {
//!         CommandId::new(MY_MODULE, "cursor-down")
//!     }
//!
//!     fn description(&self) -> &'static str {
//!         "Move cursor down"
//!     }
//!
//!     fn args(&self) -> Vec<ArgSpec> {
//!         vec![ArgSpec::optional("count", ArgKind::Count, "Number of lines")]
//!     }
//! }
//!
//! impl CommandHandler for CursorDown {
//!     fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
//!         let count = args.count().unwrap_or(1);
//!         CommandResult::Success
//!     }
//! }
//! ```

mod provider;
mod registry;
mod traits;

pub use {provider::CommandProvider, registry::CommandHandlerStore, traits::CommandHandler};

// Re-export everything from subsys-command for backwards compatibility
pub use reovim_subsys_command::*;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
