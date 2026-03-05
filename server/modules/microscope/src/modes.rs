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
mod tests {
    use super::*;

    #[test]
    fn mode_module() {
        assert_eq!(MicroscopeMode::module(), ids::MODULE);
    }

    #[test]
    fn mode_discriminant() {
        assert_eq!(MicroscopeMode::Picker.discriminant(), 0);
    }

    #[test]
    fn mode_display_name() {
        assert_eq!(MicroscopeMode::Picker.display_name(), "MICROSCOPE");
    }

    #[test]
    fn mode_cursor_style() {
        assert_eq!(MicroscopeMode::Picker.cursor_style(), CursorStyle::Bar);
    }

    #[test]
    fn mode_accepts_char_input() {
        assert!(MicroscopeMode::Picker.accepts_char_input());
    }

    #[test]
    fn mode_has_no_selection() {
        assert!(!MicroscopeMode::Picker.has_selection());
    }

    #[test]
    fn mode_no_inheritance() {
        assert!(MicroscopeMode::Picker.inherits_from().is_none());
    }

    #[test]
    fn mode_is_not_entry() {
        assert!(!MicroscopeMode::Picker.is_entry());
    }

    #[test]
    fn mode_id_matches_constant() {
        assert_eq!(MicroscopeMode::Picker.id(), MicroscopeMode::PICKER_ID);
    }

    #[test]
    fn all_modes() {
        assert_eq!(MicroscopeMode::ALL.len(), 1);
        assert_eq!(MicroscopeMode::ALL[0], MicroscopeMode::Picker);
    }

    #[test]
    fn mode_copy_clone() {
        let mode = MicroscopeMode::Picker;
        let copied = mode;
        assert_eq!(mode, copied);
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn mode_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(MicroscopeMode::Picker);
        assert!(set.contains(&MicroscopeMode::Picker));
    }

    #[test]
    fn mode_debug() {
        let debug = format!("{:?}", MicroscopeMode::Picker);
        assert!(debug.contains("Picker"));
    }
}
