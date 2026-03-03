//! Explorer mode definitions.
//!
//! Defines two modes for the explorer:
//! - `Browse` - navigation mode where keybindings handle all input
//! - `Input` - text input mode for file creation, renaming, etc.

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

use crate::ids;

/// Explorer operating modes.
///
/// `Browse` is the default mode for tree navigation.
/// `Input` is entered when the user needs to type a filename or confirm
/// an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ExplorerMode {
    /// Navigation mode - all keys resolved via keybindings.
    Browse = 0,
    /// Text input mode - characters go to the input buffer.
    Input = 1,
}

impl ExplorerMode {
    /// All explorer modes (for registration).
    pub const ALL: &'static [Self] = &[Self::Browse, Self::Input];

    /// Pre-computed `ModeId` for Browse mode.
    pub const BROWSE_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "EXPLORER", 0);

    /// Pre-computed `ModeId` for Input mode.
    pub const INPUT_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "EXPLORER_INPUT", 1);
}

impl Mode for ExplorerMode {
    fn module() -> ModuleId
    where
        Self: Sized,
    {
        ids::MODULE
    }

    fn discriminant(&self) -> u16 {
        *self as u16
    }

    fn display_name(&self) -> &'static str {
        match self {
            Self::Browse => "EXPLORER",
            Self::Input => "EXPLORER_INPUT",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        match self {
            Self::Browse => CursorStyle::Block,
            Self::Input => CursorStyle::Bar,
        }
    }

    fn accepts_char_input(&self) -> bool {
        matches!(self, Self::Input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_module() {
        assert_eq!(ExplorerMode::module(), ids::MODULE);
    }

    #[test]
    fn browse_discriminant() {
        assert_eq!(ExplorerMode::Browse.discriminant(), 0);
    }

    #[test]
    fn input_discriminant() {
        assert_eq!(ExplorerMode::Input.discriminant(), 1);
    }

    #[test]
    fn browse_display_name() {
        assert_eq!(ExplorerMode::Browse.display_name(), "EXPLORER");
    }

    #[test]
    fn input_display_name() {
        assert_eq!(ExplorerMode::Input.display_name(), "EXPLORER_INPUT");
    }

    #[test]
    fn browse_cursor_style() {
        assert_eq!(ExplorerMode::Browse.cursor_style(), CursorStyle::Block);
    }

    #[test]
    fn input_cursor_style() {
        assert_eq!(ExplorerMode::Input.cursor_style(), CursorStyle::Bar);
    }

    #[test]
    fn browse_does_not_accept_char_input() {
        assert!(!ExplorerMode::Browse.accepts_char_input());
    }

    #[test]
    fn input_accepts_char_input() {
        assert!(ExplorerMode::Input.accepts_char_input());
    }

    #[test]
    fn browse_has_no_selection() {
        assert!(!ExplorerMode::Browse.has_selection());
    }

    #[test]
    fn input_has_no_selection() {
        assert!(!ExplorerMode::Input.has_selection());
    }

    #[test]
    fn browse_no_inheritance() {
        assert!(ExplorerMode::Browse.inherits_from().is_none());
    }

    #[test]
    fn input_no_inheritance() {
        assert!(ExplorerMode::Input.inherits_from().is_none());
    }

    #[test]
    fn browse_is_not_entry() {
        assert!(!ExplorerMode::Browse.is_entry());
    }

    #[test]
    fn input_is_not_entry() {
        assert!(!ExplorerMode::Input.is_entry());
    }

    #[test]
    fn browse_mode_id_matches_constant() {
        assert_eq!(ExplorerMode::Browse.id(), ExplorerMode::BROWSE_ID);
    }

    #[test]
    fn input_mode_id_matches_constant() {
        assert_eq!(ExplorerMode::Input.id(), ExplorerMode::INPUT_ID);
    }

    #[test]
    fn all_modes() {
        assert_eq!(ExplorerMode::ALL.len(), 2);
        assert_eq!(ExplorerMode::ALL[0], ExplorerMode::Browse);
        assert_eq!(ExplorerMode::ALL[1], ExplorerMode::Input);
    }

    #[test]
    fn mode_copy_clone() {
        let mode = ExplorerMode::Browse;
        let copied = mode;
        assert_eq!(mode, copied);
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn mode_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ExplorerMode::Browse);
        set.insert(ExplorerMode::Input);
        assert!(set.contains(&ExplorerMode::Browse));
        assert!(set.contains(&ExplorerMode::Input));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn mode_debug() {
        let debug = format!("{:?}", ExplorerMode::Browse);
        assert!(debug.contains("Browse"));
        let debug = format!("{:?}", ExplorerMode::Input);
        assert!(debug.contains("Input"));
    }
}
