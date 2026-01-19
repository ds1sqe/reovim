//! Command ID constants for the undotree module.
//!
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Undotree module ID.
pub const MODULE: ModuleId = ModuleId::new("undotree");

// =============================================================================
// Navigation Commands
// =============================================================================

/// Move selection down in undotree (j).
pub const UNDOTREE_DOWN: CommandId = CommandId::new(MODULE, "undotree-down");

/// Move selection up in undotree (k).
pub const UNDOTREE_UP: CommandId = CommandId::new(MODULE, "undotree-up");

// =============================================================================
// Action Commands
// =============================================================================

/// Navigate to selected node (Enter).
pub const UNDOTREE_GOTO: CommandId = CommandId::new(MODULE, "undotree-goto");

/// Preview diff of selected node (p).
pub const UNDOTREE_PREVIEW: CommandId = CommandId::new(MODULE, "undotree-preview");

/// Close undotree panel (q, Esc).
pub const UNDOTREE_CLOSE: CommandId = CommandId::new(MODULE, "undotree-close");

/// Toggle undotree panel (:undotree).
pub const UNDOTREE: CommandId = CommandId::new(MODULE, "undotree");
