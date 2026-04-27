//! Microscope mode definition.
//!
//! Defines `MicroscopeMode::Picker` - a dedicated mode for the fuzzy finder
//! where typed characters go to the search query instead of normal editing.

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

use crate::ids;

/// Microscope picker mode.
///
/// When active, the picker captures all character input into the query field.
/// Uses `Bar` cursor style to indicate text-input context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum MicroscopeMode {
    /// The picker overlay - accepts typed characters for query input.
    Picker = 0,
}

impl MicroscopeMode {
    /// All microscope modes (for registration).
    pub const ALL: &'static [Self] = &[Self::Picker];

    /// Pre-computed `ModeId` for the picker mode.
    pub const PICKER_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "MICROSCOPE", 0);
}

impl Mode for MicroscopeMode {
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
            Self::Picker => "MICROSCOPE",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Bar
    }

    fn accepts_char_input(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "modes_tests.rs"]
mod tests;
