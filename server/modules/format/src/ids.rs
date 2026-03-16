//! Command IDs for the format module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for format.
pub const MODULE: ModuleId = ModuleId::new("format");

/// Format the current document.
pub const FORMAT_DOCUMENT: CommandId = CommandId::new(MODULE, "format-document");

/// Format the selected range.
pub const FORMAT_SELECTION: CommandId = CommandId::new(MODULE, "format-selection");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
