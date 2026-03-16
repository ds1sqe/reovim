//! Command and module IDs for the bufferline module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for bufferline.
pub const MODULE: ModuleId = ModuleId::new("bufferline");

/// Toggle pin state for the active buffer.
pub const PIN_BUFFER: CommandId = CommandId::new(MODULE, "pin");

/// Explicitly unpin the active buffer.
pub const UNPIN_BUFFER: CommandId = CommandId::new(MODULE, "unpin");

/// Close the active buffer.
pub const CLOSE_BUFFER: CommandId = CommandId::new(MODULE, "close");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
