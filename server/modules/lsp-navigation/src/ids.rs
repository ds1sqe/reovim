//! Command IDs for the LSP navigation module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for lsp-navigation.
pub const MODULE: ModuleId = ModuleId::new("lsp-navigation");

/// Go to definition (gd).
pub const GOTO_DEFINITION: CommandId = CommandId::new(MODULE, "goto-definition");

/// Find references (gr).
pub const REFERENCES: CommandId = CommandId::new(MODULE, "references");

/// Show hover information (K).
pub const HOVER: CommandId = CommandId::new(MODULE, "hover");

/// Show signature help.
pub const SIGNATURE_HELP: CommandId = CommandId::new(MODULE, "signature-help");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
