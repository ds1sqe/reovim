//! Command and mode ID constants for the snippet module (#136).
//!
//! # Constants
//!
//! - `MODULE` - Module identity
//! - `NAVIGATING_MODE` - Active snippet navigation mode
//! - Command IDs for expand, jump-next, jump-prev, cancel

use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};

// =============================================================================
// Module identity
// =============================================================================

/// Snippet module ID.
pub const MODULE: ModuleId = ModuleId::new("snippet");

// =============================================================================
// Mode IDs
// =============================================================================

/// Active snippet navigation mode.
///
/// While in this mode, Tab/S-Tab navigate between tab stops.
/// Unhandled keys fall through to vim insert mode via `inherits_from()`.
pub const NAVIGATING_MODE: ModeId = ModeId::with_discriminant(MODULE, "navigating", 0);

// =============================================================================
// Command IDs
// =============================================================================

/// Expand the snippet whose prefix is the word before the cursor.
pub const EXPAND: CommandId = CommandId::new(MODULE, "expand");

/// Jump to the next tab stop in the active snippet.
pub const JUMP_NEXT: CommandId = CommandId::new(MODULE, "jump-next");

/// Jump to the previous tab stop in the active snippet.
pub const JUMP_PREV: CommandId = CommandId::new(MODULE, "jump-prev");

/// Cancel the active snippet and return to insert mode.
pub const CANCEL: CommandId = CommandId::new(MODULE, "cancel");

/// List available snippets for the current filetype.
pub const CATALOG: CommandId = CommandId::new(MODULE, "catalog");

/// Reload all snippet files.
pub const RELOAD: CommandId = CommandId::new(MODULE, "reload");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
