//! Command and mode identifiers for the module manager (#622).

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier.
pub const MODULE: ModuleId = ModuleId::new("module-manager");

/// Open the module manager panel.
pub const OPEN: CommandId = CommandId::new(MODULE, "open");

/// Close the module manager panel.
pub const CLOSE: CommandId = CommandId::new(MODULE, "close");

/// Move selection to the next module.
pub const NEXT: CommandId = CommandId::new(MODULE, "next");

/// Move selection to the previous module.
pub const PREV: CommandId = CommandId::new(MODULE, "prev");

/// Cycle through filter views.
pub const TOGGLE_FILTER: CommandId = CommandId::new(MODULE, "toggle-filter");

/// Toggle detail view for the selected module.
pub const TOGGLE_DETAIL: CommandId = CommandId::new(MODULE, "toggle-detail");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
