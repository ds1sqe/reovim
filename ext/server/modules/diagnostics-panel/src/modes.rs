//! Diagnostics panel mode definition.
//!
//! Defines `DiagnosticsPanelMode::Panel` - a dedicated mode for navigating
//! the diagnostics list with j/k, Enter, q, and filter/sort keys.

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

use crate::ids;

/// Diagnostics panel mode.
///
/// When active, the panel captures navigation keys (j/k, Enter, q).
/// Uses `Block` cursor style since this is a list navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum DiagnosticsPanelMode {
    /// The diagnostics panel - navigable list with filter/sort.
    Panel = 0,
}

impl DiagnosticsPanelMode {
    /// All diagnostics panel modes (for registration).
    pub const ALL: &'static [Self] = &[Self::Panel];

    /// Pre-computed `ModeId` for the panel mode.
    pub const PANEL_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "DIAGNOSTICS", 0);
}

impl Mode for DiagnosticsPanelMode {
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
            Self::Panel => "DIAGNOSTICS",
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
