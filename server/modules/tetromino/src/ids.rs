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
mod tests {
    use super::*;

    #[test]
    fn module_id() {
        assert_eq!(MODULE.as_str(), "tetromino");
    }

    #[test]
    fn command_ids_have_correct_module() {
        let commands = [
            START,
            QUIT,
            MOVE_LEFT,
            MOVE_RIGHT,
            ROTATE_CW,
            ROTATE_CCW,
            SOFT_DROP,
            HARD_DROP,
            PAUSE,
            TICK,
            RESTART,
            HOLD,
            OPEN_MENU,
            START_SINGLE,
            ENTER_LOBBY,
            CREATE_ROOM,
            READY_TOGGLE,
            LEAVE_ROOM,
            LEAVE_LOBBY,
            RETURN_LOBBY,
            JOIN_ROOM_BY_INDEX,
            START_MATCH,
            ENTER_RESULT,
        ];
        for cmd in &commands {
            assert_eq!(cmd.module(), &MODULE);
        }
    }

    #[test]
    fn command_ids_are_unique() {
        let names: Vec<&str> = vec![
            START.name(),
            QUIT.name(),
            MOVE_LEFT.name(),
            MOVE_RIGHT.name(),
            ROTATE_CW.name(),
            ROTATE_CCW.name(),
            SOFT_DROP.name(),
            HARD_DROP.name(),
            PAUSE.name(),
            TICK.name(),
            RESTART.name(),
            HOLD.name(),
            OPEN_MENU.name(),
            START_SINGLE.name(),
            ENTER_LOBBY.name(),
            CREATE_ROOM.name(),
            READY_TOGGLE.name(),
            LEAVE_ROOM.name(),
            LEAVE_LOBBY.name(),
            RETURN_LOBBY.name(),
            JOIN_ROOM_BY_INDEX.name(),
            START_MATCH.name(),
            ENTER_RESULT.name(),
        ];
        let mut deduped = names.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len());
    }
}
