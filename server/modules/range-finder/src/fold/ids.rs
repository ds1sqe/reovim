//! Command ID constants for fold operations.

use reovim_kernel::api::v1::CommandId;

use crate::jump::ids::MODULE;

/// Toggle fold at cursor (`za`).
pub const FOLD_TOGGLE: CommandId = CommandId::new(MODULE, "fold-toggle");

/// Open fold at cursor (`zo`).
pub const FOLD_OPEN: CommandId = CommandId::new(MODULE, "fold-open");

/// Close fold at cursor (`zc`).
pub const FOLD_CLOSE: CommandId = CommandId::new(MODULE, "fold-close");

/// Open all folds (`zR`).
pub const FOLD_OPEN_ALL: CommandId = CommandId::new(MODULE, "fold-open-all");

/// Close all folds (`zM`).
pub const FOLD_CLOSE_ALL: CommandId = CommandId::new(MODULE, "fold-close-all");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
