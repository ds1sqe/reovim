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
#[path = "modes_tests.rs"]
mod tests;
