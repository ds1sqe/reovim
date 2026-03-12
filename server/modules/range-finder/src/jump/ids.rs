//! Command ID constants for jump navigation.

use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};

/// Range-finder module ID.
pub const MODULE: ModuleId = ModuleId::new("range-finder");

/// Start a jump search (bound to `s` in normal mode).
pub const JUMP_SEARCH: CommandId = CommandId::new(MODULE, "jump-search");

/// Execute the jump (cursor move after label resolution).
pub const JUMP_EXECUTE: CommandId = CommandId::new(MODULE, "jump-execute");

/// Start a find-char jump with pre-computed matches (from f/t motions).
///
/// Called by `ExecuteFindChar` (vim module) when multiple matches exist on a line.
/// Match positions are serialized as JSON in `CommandContext` to avoid type coupling.
pub const START_FIND_CHAR_JUMP: CommandId = CommandId::new(MODULE, "start-find-char-jump");

/// Jump-input mode ID (for label selection during jump search).
pub const JUMP_INPUT_MODE: ModeId =
    ModeId::with_discriminant(ModuleId::new("range-finder"), "jump-input", 0);

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
