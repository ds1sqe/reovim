//! Tetromino mode definitions.
//!
//! Defines five modes:
//! - `Play` - game active, keybindings control piece movement
//! - `Paused` - game paused, only unpause/quit keys work
//! - `Menu` - main menu (single player, multiplayer, quit)
//! - `Lobby` - multiplayer lobby (room list)
//! - `Room` - inside a room (ready up, leave)

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
    /// Main menu - choose single player, multiplayer, or quit.
    Menu = 2,
    /// Multiplayer lobby - view and join rooms.
    Lobby = 3,
    /// Inside a room - ready up or leave.
    Room = 4,
    /// Match result screen (multiplayer).
    Result = 5,
}

impl TetrominoMode {
    /// All tetromino modes (for registration).
    pub const ALL: &'static [Self] = &[
        Self::Play,
        Self::Paused,
        Self::Menu,
        Self::Lobby,
        Self::Room,
        Self::Result,
    ];

    /// Pre-computed `ModeId` for Play mode.
    pub const PLAY_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "PLAY", 0);

    /// Pre-computed `ModeId` for Paused mode.
    pub const PAUSED_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "PAUSED", 1);

    /// Pre-computed `ModeId` for Menu mode.
    pub const MENU_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "MENU", 2);

    /// Pre-computed `ModeId` for Lobby mode.
    pub const LOBBY_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "LOBBY", 3);

    /// Pre-computed `ModeId` for Room mode.
    pub const ROOM_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "ROOM", 4);

    /// Pre-computed `ModeId` for Result mode.
    pub const RESULT_ID: ModeId = ModeId::with_discriminant(ids::MODULE, "RESULT", 5);
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
            Self::Menu => "MENU",
            Self::Lobby => "LOBBY",
            Self::Room => "ROOM",
            Self::Result => "RESULT",
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
#[path = "modes_tests.rs"]
mod tests;
