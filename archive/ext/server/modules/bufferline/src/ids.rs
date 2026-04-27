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

/// Switch to the next buffer.
pub const NEXT_BUFFER: CommandId = CommandId::new(MODULE, "next");

/// Switch to the previous buffer.
pub const PREV_BUFFER: CommandId = CommandId::new(MODULE, "prev");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
