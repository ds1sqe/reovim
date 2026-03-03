//! Command ID constants for jump navigation.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Range-finder module ID.
pub const MODULE: ModuleId = ModuleId::new("range-finder");

/// Start a jump search (bound to `s` in normal mode).
pub const JUMP_SEARCH: CommandId = CommandId::new(MODULE, "jump-search");

/// Execute the jump (cursor move after label resolution).
pub const JUMP_EXECUTE: CommandId = CommandId::new(MODULE, "jump-execute");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        assert_eq!(MODULE.as_str(), "range-finder");
    }

    #[test]
    fn test_jump_search_id() {
        assert_eq!(JUMP_SEARCH.module(), &MODULE);
        assert_eq!(JUMP_SEARCH.name(), "jump-search");
    }

    #[test]
    fn test_jump_execute_id() {
        assert_eq!(JUMP_EXECUTE.module(), &MODULE);
        assert_eq!(JUMP_EXECUTE.name(), "jump-execute");
    }

    #[test]
    fn test_command_ids_unique() {
        assert_ne!(JUMP_SEARCH, JUMP_EXECUTE);
    }

    #[test]
    fn test_command_ids_belong_to_module() {
        assert_eq!(JUMP_SEARCH.module(), &MODULE);
        assert_eq!(JUMP_EXECUTE.module(), &MODULE);
    }
}
