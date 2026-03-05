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
            START, QUIT, MOVE_LEFT, MOVE_RIGHT, ROTATE_CW, ROTATE_CCW, SOFT_DROP, HARD_DROP, PAUSE,
            TICK, RESTART, HOLD,
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
        ];
        let mut deduped = names.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len());
    }
}
