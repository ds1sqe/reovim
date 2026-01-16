//! Undotree navigation mode.
//!
//! This mode is active when the undotree panel is focused, allowing
//! navigation through the undo history with j/k/Enter/q keybindings.

use {
    reovim_driver_display::{CursorStyle, ModeDisplay},
    reovim_driver_input::ModeInput,
    reovim_kernel::api::v1::{Mode, ModeId},
};

use crate::UNDOTREE_MODULE;

/// Undotree navigation mode.
///
/// This mode is active when the undotree panel has focus.
/// It provides navigation keybindings:
/// - `j`: Move selection down (toward children)
/// - `k`: Move selection up (toward parent)
/// - `Enter`: Go to selected node
/// - `q` / `Esc`: Close panel
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UndotreeMode;

impl UndotreeMode {
    /// Mode ID for the undotree navigation mode.
    pub const ID: ModeId = ModeId::new(UNDOTREE_MODULE, "undotree");
}

impl Mode for UndotreeMode {
    fn id(&self) -> ModeId {
        Self::ID
    }
}

impl ModeDisplay for UndotreeMode {
    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Block
    }

    fn status_text(&self) -> &'static str {
        "UNDOTREE"
    }
}

impl ModeInput for UndotreeMode {
    fn accepts_char_input(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undotree_mode_id() {
        let mode = UndotreeMode;
        assert_eq!(mode.id(), UndotreeMode::ID);
    }

    #[test]
    fn test_undotree_mode_id_module() {
        assert_eq!(UndotreeMode::ID.module(), &UNDOTREE_MODULE);
    }

    #[test]
    fn test_undotree_mode_id_name() {
        assert_eq!(UndotreeMode::ID.name(), "undotree");
    }

    #[test]
    fn test_undotree_mode_cursor_style() {
        let mode = UndotreeMode;
        assert_eq!(mode.cursor_style(), CursorStyle::Block);
    }

    #[test]
    fn test_undotree_mode_status_text() {
        let mode = UndotreeMode;
        assert_eq!(mode.status_text(), "UNDOTREE");
    }

    #[test]
    fn test_undotree_mode_accepts_char_input() {
        let mode = UndotreeMode;
        assert!(!mode.accepts_char_input());
    }
}
