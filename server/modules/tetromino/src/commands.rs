//! Command handlers for the tetromino game.
//!
//! Each command is a zero-sized struct implementing `Command` + `CommandHandler`.
//! The `execute` methods use `SessionRuntime` and are marked `coverage(off)`
//! since they require full runtime infrastructure to test.

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{ExtensionApi, ModeApi, SessionRuntime, TransitionContext},
    reovim_kernel::api::v1::CommandId,
};

use crate::{game, ids, modes::TetrominoMode, state::TetrominoState};

/// Vim normal mode ID constant.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn vim_normal() -> reovim_kernel::api::v1::ModeId {
    reovim_kernel::api::v1::ModeId::with_discriminant(
        reovim_kernel::api::v1::ModuleId::new("vim"),
        "NORMAL",
        0,
    )
}

// ============================================================================
// Start / Quit / Restart
// ============================================================================

/// Start a new tetromino game.
#[derive(Debug, Clone, Copy, Default)]
pub struct Start;

impl reovim_driver_command::Command for Start {
    fn id(&self) -> CommandId {
        ids::START
    }

    fn description(&self) -> &'static str {
        "Start tetromino game"
    }
}

impl CommandHandler for Start {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.start_game();
        runtime.set_mode(TetrominoMode::PLAY_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Quit the tetromino game.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quit;

impl reovim_driver_command::Command for Quit {
    fn id(&self) -> CommandId {
        ids::QUIT
    }

    fn description(&self) -> &'static str {
        "Quit tetromino game"
    }
}

impl CommandHandler for Quit {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.active = false;
        state.game = None;
        runtime.set_mode(vim_normal(), TransitionContext::new());
        CommandResult::Success
    }
}

/// Restart the tetromino game.
#[derive(Debug, Clone, Copy, Default)]
pub struct Restart;

impl reovim_driver_command::Command for Restart {
    fn id(&self) -> CommandId {
        ids::RESTART
    }

    fn description(&self) -> &'static str {
        "Restart tetromino game"
    }
}

impl CommandHandler for Restart {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.start_game();
        runtime.set_mode(TetrominoMode::PLAY_ID, TransitionContext::new());
        CommandResult::Success
    }
}

// ============================================================================
// Movement
// ============================================================================

/// Move piece left.
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveLeft;

impl reovim_driver_command::Command for MoveLeft {
    fn id(&self) -> CommandId {
        ids::MOVE_LEFT
    }

    fn description(&self) -> &'static str {
        "Move piece left"
    }
}

impl CommandHandler for MoveLeft {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if let Some(ref mut game_state) = state.game
            && let Some(ref piece) = game_state.active_piece
            && let Some(moved) = game::try_move_left(&game_state.board, piece)
        {
            game_state.active_piece = Some(moved);
        }
        CommandResult::Success
    }
}

/// Move piece right.
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveRight;

impl reovim_driver_command::Command for MoveRight {
    fn id(&self) -> CommandId {
        ids::MOVE_RIGHT
    }

    fn description(&self) -> &'static str {
        "Move piece right"
    }
}

impl CommandHandler for MoveRight {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if let Some(ref mut game_state) = state.game
            && let Some(ref piece) = game_state.active_piece
            && let Some(moved) = game::try_move_right(&game_state.board, piece)
        {
            game_state.active_piece = Some(moved);
        }
        CommandResult::Success
    }
}

/// Rotate piece clockwise.
#[derive(Debug, Clone, Copy, Default)]
pub struct RotateCw;

impl reovim_driver_command::Command for RotateCw {
    fn id(&self) -> CommandId {
        ids::ROTATE_CW
    }

    fn description(&self) -> &'static str {
        "Rotate piece clockwise"
    }
}

impl CommandHandler for RotateCw {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if let Some(ref mut game_state) = state.game
            && let Some(ref piece) = game_state.active_piece
            && let Some(rotated) = game::try_rotate_cw(&game_state.board, piece)
        {
            game_state.active_piece = Some(rotated);
        }
        CommandResult::Success
    }
}

/// Rotate piece counter-clockwise.
#[derive(Debug, Clone, Copy, Default)]
pub struct RotateCcw;

impl reovim_driver_command::Command for RotateCcw {
    fn id(&self) -> CommandId {
        ids::ROTATE_CCW
    }

    fn description(&self) -> &'static str {
        "Rotate piece counter-clockwise"
    }
}

impl CommandHandler for RotateCcw {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if let Some(ref mut game_state) = state.game
            && let Some(ref piece) = game_state.active_piece
            && let Some(rotated) = game::try_rotate_ccw(&game_state.board, piece)
        {
            game_state.active_piece = Some(rotated);
        }
        CommandResult::Success
    }
}

/// Soft drop (move piece down one row).
#[derive(Debug, Clone, Copy, Default)]
pub struct SoftDrop;

impl reovim_driver_command::Command for SoftDrop {
    fn id(&self) -> CommandId {
        ids::SOFT_DROP
    }

    fn description(&self) -> &'static str {
        "Soft drop piece"
    }
}

impl CommandHandler for SoftDrop {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if let Some(ref mut game_state) = state.game
            && let Some(ref piece) = game_state.active_piece
        {
            if let Some(moved) = game::try_move_down(&game_state.board, piece) {
                game_state.active_piece = Some(moved);
                game_state.score += 2; // 2 points per soft drop cell
            } else {
                let piece = game_state.active_piece.clone().unwrap();
                state.lock_clear_spawn(&piece);
            }
        }
        CommandResult::Success
    }
}

/// Hard drop (drop piece to bottom and lock).
#[derive(Debug, Clone, Copy, Default)]
pub struct HardDrop;

impl reovim_driver_command::Command for HardDrop {
    fn id(&self) -> CommandId {
        ids::HARD_DROP
    }

    fn description(&self) -> &'static str {
        "Hard drop piece"
    }
}

impl CommandHandler for HardDrop {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if let Some(ref mut game_state) = state.game
            && let Some(ref piece) = game_state.active_piece
        {
            let dropped = game::hard_drop(&game_state.board, piece);
            // 3 points per cell dropped
            #[allow(clippy::cast_sign_loss)]
            let drop_distance = (dropped.row - piece.row) as u32;
            game_state.score += drop_distance * 3;
            game_state.active_piece = Some(dropped.clone());
            state.lock_clear_spawn(&dropped);
        }
        CommandResult::Success
    }
}

/// Hold current piece.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hold;

impl reovim_driver_command::Command for Hold {
    fn id(&self) -> CommandId {
        ids::HOLD
    }

    fn description(&self) -> &'static str {
        "Hold current piece"
    }
}

impl CommandHandler for Hold {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.hold_piece();
        CommandResult::Success
    }
}

// ============================================================================
// Pause / Tick
// ============================================================================

/// Toggle pause state.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pause;

impl reovim_driver_command::Command for Pause {
    fn id(&self) -> CommandId {
        ids::PAUSE
    }

    fn description(&self) -> &'static str {
        "Toggle pause"
    }
}

impl CommandHandler for Pause {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if state.paused {
            state.paused = false;
            state.last_tick = std::time::Instant::now();
            runtime.set_mode(TetrominoMode::PLAY_ID, TransitionContext::new());
        } else {
            state.paused = true;
            runtime.set_mode(TetrominoMode::PAUSED_ID, TransitionContext::new());
        }
        CommandResult::Success
    }
}

/// Advance game state (gravity tick).
#[derive(Debug, Clone, Copy, Default)]
pub struct Tick;

impl reovim_driver_command::Command for Tick {
    fn id(&self) -> CommandId {
        ids::TICK
    }

    fn description(&self) -> &'static str {
        "Gravity tick"
    }
}

impl CommandHandler for Tick {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if state.should_tick() {
            state.apply_tick();
        }
        CommandResult::Success
    }
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(Start),
        Box::new(Quit),
        Box::new(Restart),
        Box::new(MoveLeft),
        Box::new(MoveRight),
        Box::new(RotateCw),
        Box::new(RotateCcw),
        Box::new(SoftDrop),
        Box::new(HardDrop),
        Box::new(Hold),
        Box::new(Pause),
        Box::new(Tick),
    ]
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[test]
    fn start_metadata() {
        let cmd = Start;
        assert_eq!(cmd.id(), ids::START);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn quit_metadata() {
        let cmd = Quit;
        assert_eq!(cmd.id(), ids::QUIT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn restart_metadata() {
        let cmd = Restart;
        assert_eq!(cmd.id(), ids::RESTART);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn move_left_metadata() {
        let cmd = MoveLeft;
        assert_eq!(cmd.id(), ids::MOVE_LEFT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn move_right_metadata() {
        let cmd = MoveRight;
        assert_eq!(cmd.id(), ids::MOVE_RIGHT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn rotate_cw_metadata() {
        let cmd = RotateCw;
        assert_eq!(cmd.id(), ids::ROTATE_CW);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn soft_drop_metadata() {
        let cmd = SoftDrop;
        assert_eq!(cmd.id(), ids::SOFT_DROP);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn hard_drop_metadata() {
        let cmd = HardDrop;
        assert_eq!(cmd.id(), ids::HARD_DROP);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn pause_metadata() {
        let cmd = Pause;
        assert_eq!(cmd.id(), ids::PAUSE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn tick_metadata() {
        let cmd = Tick;
        assert_eq!(cmd.id(), ids::TICK);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn rotate_ccw_metadata() {
        let cmd = RotateCcw;
        assert_eq!(cmd.id(), ids::ROTATE_CCW);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn hold_metadata() {
        let cmd = Hold;
        assert_eq!(cmd.id(), ids::HOLD);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_count() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 12);
    }

    #[test]
    fn command_handlers_unique_ids() {
        let handlers = command_handlers();
        let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
        let mut deduped = ids.clone();
        deduped.sort_by_key(CommandId::name);
        deduped.dedup_by_key(|id| id.name());
        assert_eq!(ids.len(), deduped.len());
    }

    #[test]
    fn all_debug() {
        let debug = format!("{Start:?}");
        assert!(debug.contains("Start"));
        let debug = format!("{Quit:?}");
        assert!(debug.contains("Quit"));
        let debug = format!("{MoveLeft:?}");
        assert!(debug.contains("MoveLeft"));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn all_default_constructable() {
        let _ = Start::default();
        let _ = Quit::default();
        let _ = Restart::default();
        let _ = MoveLeft::default();
        let _ = MoveRight::default();
        let _ = RotateCw::default();
        let _ = RotateCcw::default();
        let _ = SoftDrop::default();
        let _ = HardDrop::default();
        let _ = Hold::default();
        let _ = Pause::default();
        let _ = Tick::default();
    }
}
