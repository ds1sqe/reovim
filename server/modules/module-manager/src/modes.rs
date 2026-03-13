//! Module manager mode definition (#622).
//!
//! Defines `ManagerMode::Manager` - a navigation-only mode for the module list
//! where j/k move selection and Tab cycles filters.

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

use crate::ids;

/// Module manager panel mode.
///
/// When active, the manager captures navigation keys (j/k, Tab, Enter, Esc).
/// No character input - this is a list navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ManagerMode {
    /// The manager panel - navigates module list.
    Manager = 0,
}

impl ManagerMode {
    /// All module manager modes (for registration).
    pub const ALL: &'static [Self] = &[Self::Manager];

    /// Pre-computed `ModeId` for the manager mode.
    pub const MANAGER_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "MANAGER", 0);
}

impl Mode for ManagerMode {
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
            Self::Manager => "MANAGER",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Block
    }

    fn accepts_char_input(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "modes_tests.rs"]
mod tests;
