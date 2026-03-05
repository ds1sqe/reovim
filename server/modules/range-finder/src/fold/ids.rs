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
mod tests {
    use super::*;

    #[test]
    fn test_fold_toggle_id() {
        assert_eq!(FOLD_TOGGLE.module(), &MODULE);
        assert_eq!(FOLD_TOGGLE.name(), "fold-toggle");
    }

    #[test]
    fn test_fold_open_id() {
        assert_eq!(FOLD_OPEN.module(), &MODULE);
        assert_eq!(FOLD_OPEN.name(), "fold-open");
    }

    #[test]
    fn test_fold_close_id() {
        assert_eq!(FOLD_CLOSE.module(), &MODULE);
        assert_eq!(FOLD_CLOSE.name(), "fold-close");
    }

    #[test]
    fn test_fold_open_all_id() {
        assert_eq!(FOLD_OPEN_ALL.module(), &MODULE);
        assert_eq!(FOLD_OPEN_ALL.name(), "fold-open-all");
    }

    #[test]
    fn test_fold_close_all_id() {
        assert_eq!(FOLD_CLOSE_ALL.module(), &MODULE);
        assert_eq!(FOLD_CLOSE_ALL.name(), "fold-close-all");
    }

    #[test]
    fn test_fold_command_ids_unique() {
        let ids = [
            FOLD_TOGGLE,
            FOLD_OPEN,
            FOLD_CLOSE,
            FOLD_OPEN_ALL,
            FOLD_CLOSE_ALL,
        ];
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j]);
            }
        }
    }

    #[test]
    fn test_fold_command_ids_belong_to_module() {
        for id in [
            FOLD_TOGGLE,
            FOLD_OPEN,
            FOLD_CLOSE,
            FOLD_OPEN_ALL,
            FOLD_CLOSE_ALL,
        ] {
            assert_eq!(id.module(), &MODULE);
        }
    }
}
