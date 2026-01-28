//! Undotree navigation mode.
//!
//! This mode is active when the undotree panel is focused, allowing
//! navigation through the undo history with j/k/Enter/q keybindings.

use {
    reovim_driver_display::ModeDisplay,
    reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId},
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
    pub const ID: ModeId = ModeId::with_discriminant(UNDOTREE_MODULE, "UNDOTREE", 0);
}

impl Mode for UndotreeMode {
    fn module() -> ModuleId {
        UNDOTREE_MODULE
    }

    fn discriminant(&self) -> u16 {
        0
    }

    fn id(&self) -> ModeId {
        Self::ID
    }

    fn display_name(&self) -> &'static str {
        "UNDOTREE"
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Block
    }

    fn accepts_char_input(&self) -> bool {
        false
    }
}

impl ModeDisplay for UndotreeMode {
    fn cursor_style(&self) -> CursorStyle {
        Mode::cursor_style(self)
    }

    fn status_text(&self) -> &'static str {
        Mode::display_name(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undotree_mode_id() {
        let mode = UndotreeMode;
        assert_eq!(Mode::id(&mode), UndotreeMode::ID);
    }

    #[test]
    fn test_undotree_mode_id_module() {
        assert_eq!(UndotreeMode::ID.module(), &UNDOTREE_MODULE);
    }

    #[test]
    fn test_undotree_mode_id_name() {
        assert_eq!(UndotreeMode::ID.name(), "UNDOTREE");
    }

    #[test]
    fn test_undotree_mode_cursor_style() {
        let mode = UndotreeMode;
        assert_eq!(<UndotreeMode as Mode>::cursor_style(&mode), CursorStyle::Block);
    }

    #[test]
    fn test_undotree_mode_status_text() {
        let mode = UndotreeMode;
        assert_eq!(<UndotreeMode as Mode>::display_name(&mode), "UNDOTREE");
    }

    #[test]
    fn test_undotree_mode_accepts_char_input() {
        let mode = UndotreeMode;
        assert!(!<UndotreeMode as Mode>::accepts_char_input(&mode));
    }
}
