#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Command driver for reovim - command execution framework.
//!
//! Linux equivalent: `drivers/block/` (block command interface)
//!
//! # Architecture
//!
//! This crate defines the command execution framework for reovim.
//! Commands implement [`Command`] for metadata and [`CommandHandler`] for execution.
//!
//! ```text
//! server/lib/drivers/command/  <-- Command framework (this crate)
//!        ^
//!        |  (modules implement commands)
//!        |
//! server/modules/              <-- Policy: actual command implementations
//! ```
//!
//! # Components
//!
//! - [`Command`] - Self-describing command metadata
//! - [`CommandHandler`] - Command execution trait
//! - [`ArgSpec`] - Argument specification
//! - [`ArgKind`], [`ArgValue`] - Argument types
//! - [`CommandContext`] - Context carrying all command inputs
//! - [`CommandResult`] - Command execution result
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
//!         // Move cursor down by count lines using runtime.kernel() escape hatch
//!         // or BufferApi methods
//!         CommandResult::Success
//!     }
//! }
//! ```

// Internal modules
mod name_index;
mod parse;
mod provider;
mod query;
mod registry;
mod traits;

// Re-export from command-types for backwards compatibility
pub use reovim_driver_command_types::{
    ArgKind, ArgSpec, ArgValue, CommandContext, CommandResult, MotionType, RuntimeSignal,
};

// Re-export provider trait
pub use provider::CommandProvider;

// Re-export registry for ServiceRegistry (Epic #417 Part 3)
pub use registry::CommandHandlerStore;

// Re-export query service (#453, #522)
pub use query::{CommandInfo, CommandQueryProvider, CommandQueryService};

// Re-export name index and cmdline parser (#547, #559, #561)
pub use {
    name_index::{AmbiguousPrefix, CommandNameIndex},
    parse::{ArgError, ParsedCmdline, bind_args, parse_cmdline, tokenize_args},
};

// Re-export traits
pub use traits::{Command, CommandHandler, CommandPriority};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
