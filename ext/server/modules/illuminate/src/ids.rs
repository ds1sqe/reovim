//! Command and module identifiers for the illuminate module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for illuminate.
pub const MODULE: ModuleId = ModuleId::new("illuminate");

/// Navigate to the next highlighted reference.
pub const NEXT_REFERENCE: CommandId = CommandId::new(MODULE, "next-reference");

/// Navigate to the previous highlighted reference.
pub const PREV_REFERENCE: CommandId = CommandId::new(MODULE, "prev-reference");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
