//! Tetromino mode definitions.
//!
//! Defines two modes:
//! - `Play` - game active, keybindings control piece movement
//! - `Paused` - game paused, only unpause/quit keys work

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

use crate::ids;

/// Tetromino operating modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum TetrominoMode {
    /// Active gameplay - keys control piece movement and rotation.
    Play = 0,
    /// Paused - only unpause and quit keys are active.
    Paused = 1,
}

impl TetrominoMode {
    /// All tetromino modes (for registration).
    pub const ALL: &'static [Self] = &[Self::Play, Self::Paused];

    /// Pre-computed `ModeId` for Play mode.
    pub const PLAY_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "PLAY", 0);

    /// Pre-computed `ModeId` for Paused mode.
    pub const PAUSED_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "PAUSED", 1);
}

impl Mode for TetrominoMode {
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
            Self::Play => "PLAY",
            Self::Paused => "PAUSED",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Hidden
    }

    fn accepts_char_input(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_module() {
        assert_eq!(TetrominoMode::module(), ids::MODULE);
    }

    #[test]
    fn play_discriminant() {
        assert_eq!(TetrominoMode::Play.discriminant(), 0);
    }

    #[test]
    fn paused_discriminant() {
        assert_eq!(TetrominoMode::Paused.discriminant(), 1);
    }

    #[test]
    fn play_display_name() {
        assert_eq!(TetrominoMode::Play.display_name(), "PLAY");
    }

    #[test]
    fn paused_display_name() {
        assert_eq!(TetrominoMode::Paused.display_name(), "PAUSED");
    }

    #[test]
    fn play_cursor_hidden() {
        assert_eq!(TetrominoMode::Play.cursor_style(), CursorStyle::Hidden);
    }

    #[test]
    fn paused_cursor_hidden() {
        assert_eq!(TetrominoMode::Paused.cursor_style(), CursorStyle::Hidden);
    }

    #[test]
    fn play_no_char_input() {
        assert!(!TetrominoMode::Play.accepts_char_input());
    }

    #[test]
    fn paused_no_char_input() {
        assert!(!TetrominoMode::Paused.accepts_char_input());
    }

    #[test]
    fn play_no_selection() {
        assert!(!TetrominoMode::Play.has_selection());
    }

    #[test]
    fn paused_no_selection() {
        assert!(!TetrominoMode::Paused.has_selection());
    }

    #[test]
    fn play_no_inheritance() {
        assert!(TetrominoMode::Play.inherits_from().is_none());
    }

    #[test]
    fn paused_no_inheritance() {
        assert!(TetrominoMode::Paused.inherits_from().is_none());
    }

    #[test]
    fn play_not_entry() {
        assert!(!TetrominoMode::Play.is_entry());
    }

    #[test]
    fn paused_not_entry() {
        assert!(!TetrominoMode::Paused.is_entry());
    }

    #[test]
    fn play_mode_id_matches_constant() {
        assert_eq!(TetrominoMode::Play.id(), TetrominoMode::PLAY_ID);
    }

    #[test]
    fn paused_mode_id_matches_constant() {
        assert_eq!(TetrominoMode::Paused.id(), TetrominoMode::PAUSED_ID);
    }

    #[test]
    fn all_modes() {
        assert_eq!(TetrominoMode::ALL.len(), 2);
        assert_eq!(TetrominoMode::ALL[0], TetrominoMode::Play);
        assert_eq!(TetrominoMode::ALL[1], TetrominoMode::Paused);
    }

    #[test]
    fn mode_copy_clone() {
        let mode = TetrominoMode::Play;
        let copied = mode;
        assert_eq!(mode, copied);
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn mode_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TetrominoMode::Play);
        set.insert(TetrominoMode::Paused);
        assert!(set.contains(&TetrominoMode::Play));
        assert!(set.contains(&TetrominoMode::Paused));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn mode_debug() {
        let debug = format!("{:?}", TetrominoMode::Play);
        assert!(debug.contains("Play"));
        let debug = format!("{:?}", TetrominoMode::Paused);
        assert!(debug.contains("Paused"));
    }
}
