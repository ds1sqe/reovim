//! Command and module identifiers for the tetromino module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for tetromino.
pub const MODULE: ModuleId = ModuleId::new("tetromino");

/// Start a new game.
pub const START: CommandId = CommandId::new(MODULE, "start");

/// Quit the game.
pub const QUIT: CommandId = CommandId::new(MODULE, "quit");

/// Move piece left.
pub const MOVE_LEFT: CommandId = CommandId::new(MODULE, "move-left");

/// Move piece right.
pub const MOVE_RIGHT: CommandId = CommandId::new(MODULE, "move-right");

/// Rotate piece clockwise.
pub const ROTATE_CW: CommandId = CommandId::new(MODULE, "rotate-cw");

/// Rotate piece counter-clockwise.
pub const ROTATE_CCW: CommandId = CommandId::new(MODULE, "rotate-ccw");

/// Hold current piece.
pub const HOLD: CommandId = CommandId::new(MODULE, "hold");

/// Soft drop (move piece down one row).
pub const SOFT_DROP: CommandId = CommandId::new(MODULE, "soft-drop");

/// Hard drop (drop piece to bottom).
pub const HARD_DROP: CommandId = CommandId::new(MODULE, "hard-drop");

/// Toggle pause.
pub const PAUSE: CommandId = CommandId::new(MODULE, "pause");

/// Advance game state (gravity tick).
pub const TICK: CommandId = CommandId::new(MODULE, "tick");

/// Restart the game.
pub const RESTART: CommandId = CommandId::new(MODULE, "restart");

/// Open the tetromino menu.
pub const OPEN_MENU: CommandId = CommandId::new(MODULE, "open-menu");

/// Start a single-player game from the menu.
pub const START_SINGLE: CommandId = CommandId::new(MODULE, "start-single");

/// Enter the multiplayer lobby.
pub const ENTER_LOBBY: CommandId = CommandId::new(MODULE, "enter-lobby");

/// Create a new multiplayer room.
pub const CREATE_ROOM: CommandId = CommandId::new(MODULE, "create-room");

/// Toggle ready status in a room.
pub const READY_TOGGLE: CommandId = CommandId::new(MODULE, "ready-toggle");

/// Leave the current room.
pub const LEAVE_ROOM: CommandId = CommandId::new(MODULE, "leave-room");

/// Leave the lobby and return to the menu.
pub const LEAVE_LOBBY: CommandId = CommandId::new(MODULE, "leave-lobby");

/// Return to lobby from the result screen.
pub const RETURN_LOBBY: CommandId = CommandId::new(MODULE, "return-lobby");

/// Join a room by digit-key index (from lobby resolver).
pub const JOIN_ROOM_BY_INDEX: CommandId = CommandId::new(MODULE, "join-room-by-index");

/// Start a multiplayer match (from room resolver).
pub const START_MATCH: CommandId = CommandId::new(MODULE, "start-match");

/// Enter the result screen (from play resolver/tick).
pub const ENTER_RESULT: CommandId = CommandId::new(MODULE, "enter-result");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
