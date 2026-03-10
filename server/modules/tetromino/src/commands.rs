//! Command handlers for the polyblocks game.
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

/// Tick scheduler polling interval (50ms = 20 Hz).
///
/// The actual gravity speed is controlled by `should_tick()` which checks
/// `tick_interval_ms(level)` internally. The scheduler just polls frequently;
/// the bridge decides whether to actually advance.
const TICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// Start the tick scheduler for a client.
#[cfg_attr(coverage_nightly, coverage(off))]
fn start_tick(runtime: &SessionRuntime<'_>) {
    if let (Some(client_id), Some(handle)) = (
        runtime.owner(),
        runtime
            .kernel()
            .services
            .get::<reovim_driver_session::TickSchedulerHandle>(),
    ) {
        handle.start(client_id, "polyblocks", TICK_INTERVAL);
    }
}

/// Start the tick scheduler for all players in a room.
#[cfg_attr(coverage_nightly, coverage(off))]
fn start_tick_for_players(
    runtime: &SessionRuntime<'_>,
    players: &[reovim_driver_session::ClientId],
) {
    if let Some(handle) = runtime
        .kernel()
        .services
        .get::<reovim_driver_session::TickSchedulerHandle>()
    {
        for &player_id in players {
            handle.start(player_id, "polyblocks", TICK_INTERVAL);
        }
    }
}

/// Stop the tick scheduler for a client.
#[cfg_attr(coverage_nightly, coverage(off))]
fn stop_tick(runtime: &SessionRuntime<'_>) {
    if let (Some(client_id), Some(handle)) = (
        runtime.owner(),
        runtime
            .kernel()
            .services
            .get::<reovim_driver_session::TickSchedulerHandle>(),
    ) {
        handle.stop(client_id, "polyblocks");
    }
}

/// Pause the tick scheduler for a client.
#[cfg_attr(coverage_nightly, coverage(off))]
fn pause_tick(runtime: &SessionRuntime<'_>) {
    if let (Some(client_id), Some(handle)) = (
        runtime.owner(),
        runtime
            .kernel()
            .services
            .get::<reovim_driver_session::TickSchedulerHandle>(),
    ) {
        handle.pause(client_id, "polyblocks");
    }
}

/// Resume the tick scheduler for a client.
#[cfg_attr(coverage_nightly, coverage(off))]
fn resume_tick(runtime: &SessionRuntime<'_>) {
    if let (Some(client_id), Some(handle)) = (
        runtime.owner(),
        runtime
            .kernel()
            .services
            .get::<reovim_driver_session::TickSchedulerHandle>(),
    ) {
        handle.resume(client_id, "polyblocks");
    }
}

// ============================================================================
// Open Menu / Start Single / Quit / Restart
// ============================================================================

/// Open the tetromino main menu.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenMenu;

impl reovim_driver_command::Command for OpenMenu {
    fn id(&self) -> CommandId {
        ids::OPEN_MENU
    }

    fn description(&self) -> &'static str {
        "Open polyblocks menu"
    }

    fn names(&self) -> &[&'static str] {
        &["polyblocks"]
    }
}

impl CommandHandler for OpenMenu {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let client_id = runtime.owner();
        let state = runtime.ext_mut::<TetrominoState>();
        state.active = true;
        state.screen = crate::state::TetrominoScreen::Menu;
        state.client_id = client_id;
        runtime.push_mode(TetrominoMode::MENU_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Start a new polyblocks game (kept for backward compatibility).
#[derive(Debug, Clone, Copy, Default)]
pub struct Start;

impl reovim_driver_command::Command for Start {
    fn id(&self) -> CommandId {
        ids::START
    }

    fn description(&self) -> &'static str {
        "Start polyblocks game"
    }
}

impl CommandHandler for Start {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.start_game();
        state.screen = crate::state::TetrominoScreen::Game;
        state.multiplayer = false;
        runtime.set_mode(TetrominoMode::PLAY_ID, TransitionContext::new());
        start_tick(runtime);
        CommandResult::Success
    }
}

/// Start a single-player game from the menu.
#[derive(Debug, Clone, Copy, Default)]
pub struct StartSingle;

impl reovim_driver_command::Command for StartSingle {
    fn id(&self) -> CommandId {
        ids::START_SINGLE
    }

    fn description(&self) -> &'static str {
        "Start single player"
    }
}

impl CommandHandler for StartSingle {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.screen = crate::state::TetrominoScreen::Game;
        state.multiplayer = false;
        state.start_game();
        runtime.set_mode(TetrominoMode::PLAY_ID, TransitionContext::new());
        start_tick(runtime);
        CommandResult::Success
    }
}

/// Quit the polyblocks game.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quit;

impl reovim_driver_command::Command for Quit {
    fn id(&self) -> CommandId {
        ids::QUIT
    }

    fn description(&self) -> &'static str {
        "Quit polyblocks game"
    }
}

impl CommandHandler for Quit {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        stop_tick(runtime);

        // Extract room info before mutating state
        let room_id = {
            let state = runtime.ext::<TetrominoState>();
            state.and_then(|s| s.current_room)
        };
        let client_id = runtime.owner();

        // Clean up room membership if in a room
        if let Some(rid) = room_id
            && let (Some(cid), Some(lobby)) =
                (client_id, runtime.shared_ext_mut::<crate::state::TetrominoLobbyState>())
        {
            // Treat quit during active match as forfeit
            if lobby.is_in_progress(rid) || lobby.is_countdown(rid) {
                lobby.mark_dead(rid, cid);
                if lobby.alive_count(rid) <= 1
                    && let Some(winner) = lobby.last_alive(rid)
                {
                    lobby.finish_match(rid, winner);
                }
            }
            lobby.leave_room(rid, cid);
        }

        let state = runtime.ext_mut::<TetrominoState>();
        state.active = false;
        state.game = None;
        state.screen = crate::state::TetrominoScreen::Menu;
        state.multiplayer = false;
        state.current_room = None;
        let _ = runtime.pop_mode(None);
        CommandResult::Success
    }
}

/// Restart the polyblocks game.
#[derive(Debug, Clone, Copy, Default)]
pub struct Restart;

impl reovim_driver_command::Command for Restart {
    fn id(&self) -> CommandId {
        ids::RESTART
    }

    fn description(&self) -> &'static str {
        "Restart polyblocks game"
    }
}

impl CommandHandler for Restart {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        if state.multiplayer {
            return CommandResult::Success; // disabled in multiplayer
        }
        state.start_game();
        runtime.set_mode(TetrominoMode::PLAY_ID, TransitionContext::new());
        stop_tick(runtime);
        start_tick(runtime);
        CommandResult::Success
    }
}

// ============================================================================
// Lobby / Room
// ============================================================================

/// Enter the multiplayer lobby.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterLobby;

impl reovim_driver_command::Command for EnterLobby {
    fn id(&self) -> CommandId {
        ids::ENTER_LOBBY
    }

    fn description(&self) -> &'static str {
        "Enter multiplayer lobby"
    }
}

impl CommandHandler for EnterLobby {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.screen = crate::state::TetrominoScreen::Lobby;
        runtime.set_mode(TetrominoMode::LOBBY_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Create a new multiplayer room.
#[derive(Debug, Clone, Copy, Default)]
pub struct CreateRoom;

impl reovim_driver_command::Command for CreateRoom {
    fn id(&self) -> CommandId {
        ids::CREATE_ROOM
    }

    fn description(&self) -> &'static str {
        "Create multiplayer room"
    }
}

impl CommandHandler for CreateRoom {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let client_id = runtime.owner().expect("client context");
        let lobby = runtime
            .shared_ext_mut::<crate::state::TetrominoLobbyState>()
            .expect("shared extensions available");
        let room_id = lobby.create_room();
        lobby.join_room(room_id, client_id);

        let state = runtime.ext_mut::<TetrominoState>();
        state.current_room = Some(room_id);
        state.screen = crate::state::TetrominoScreen::Room;
        runtime.set_mode(TetrominoMode::ROOM_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Toggle ready status in a room.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadyToggle;

impl reovim_driver_command::Command for ReadyToggle {
    fn id(&self) -> CommandId {
        ids::READY_TOGGLE
    }

    fn description(&self) -> &'static str {
        "Toggle ready status"
    }
}

impl CommandHandler for ReadyToggle {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Extract room_id from per-client state
        let room_id = {
            let state = runtime.ext::<TetrominoState>();
            state.and_then(|s| s.current_room)
        };
        let Some(room_id) = room_id else {
            return CommandResult::Success;
        };
        let client_id = runtime.owner().expect("client context");

        let lobby = runtime
            .shared_ext_mut::<crate::state::TetrominoLobbyState>()
            .expect("shared extensions available");
        let all_ready = lobby.toggle_ready(room_id, client_id);

        if all_ready == Some(true) {
            // All ready → start countdown (game begins after 3s)
            let players = lobby.start_countdown(room_id);
            let state = runtime.ext_mut::<TetrominoState>();
            state.screen = crate::state::TetrominoScreen::Countdown;
            // Start tick scheduler for ALL players (tick handles countdown → game)
            start_tick_for_players(runtime, &players);
        }
        CommandResult::Success
    }
}

/// Leave the current room.
#[derive(Debug, Clone, Copy, Default)]
pub struct LeaveRoom;

impl reovim_driver_command::Command for LeaveRoom {
    fn id(&self) -> CommandId {
        ids::LEAVE_ROOM
    }

    fn description(&self) -> &'static str {
        "Leave room"
    }
}

impl CommandHandler for LeaveRoom {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Extract room_id before shared_ext_mut
        let room_id = {
            let state = runtime.ext_mut::<TetrominoState>();
            state.current_room.take()
        };

        if let Some(rid) = room_id {
            let client_id = runtime.owner().expect("client context");
            if let Some(lobby) = runtime.shared_ext_mut::<crate::state::TetrominoLobbyState>() {
                lobby.leave_room(rid, client_id);
            }
        }

        let state = runtime.ext_mut::<TetrominoState>();
        state.screen = crate::state::TetrominoScreen::Lobby;
        runtime.set_mode(TetrominoMode::LOBBY_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Leave the lobby and return to the menu.
#[derive(Debug, Clone, Copy, Default)]
pub struct LeaveLobby;

impl reovim_driver_command::Command for LeaveLobby {
    fn id(&self) -> CommandId {
        ids::LEAVE_LOBBY
    }

    fn description(&self) -> &'static str {
        "Leave lobby"
    }
}

impl CommandHandler for LeaveLobby {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.screen = crate::state::TetrominoScreen::Menu;
        runtime.set_mode(TetrominoMode::MENU_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Return to the lobby from the result screen.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReturnLobby;

impl reovim_driver_command::Command for ReturnLobby {
    fn id(&self) -> CommandId {
        ids::RETURN_LOBBY
    }

    fn description(&self) -> &'static str {
        "Return to lobby"
    }
}

impl CommandHandler for ReturnLobby {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<TetrominoState>();
        state.screen = crate::state::TetrominoScreen::Lobby;
        state.game = None;
        state.current_room = None;
        state.multiplayer = false;
        runtime.set_mode(TetrominoMode::LOBBY_ID, TransitionContext::new());
        CommandResult::Success
    }
}

// ============================================================================
// Resolver-delegated mode transitions
// ============================================================================

/// Join a room by digit-key index (delegated from `LobbyResolver`).
///
/// The resolver sets `current_room` and `screen = Room` in client extensions
/// before returning `Execute(JOIN_ROOM_BY_INDEX)`. This handler only performs
/// the safe `set_mode()` call.
#[derive(Debug, Clone, Copy, Default)]
pub struct JoinRoomByIndex;

impl reovim_driver_command::Command for JoinRoomByIndex {
    fn id(&self) -> CommandId {
        ids::JOIN_ROOM_BY_INDEX
    }

    fn description(&self) -> &'static str {
        "Join room by index"
    }
}

impl CommandHandler for JoinRoomByIndex {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.set_mode(TetrominoMode::ROOM_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Start a multiplayer match (delegated from `RoomResolver`).
///
/// The resolver sets `screen = Game`, `multiplayer = true`, and calls
/// `start_game()` before returning `Execute(START_MATCH)`. This handler
/// starts the tick scheduler and performs the safe `set_mode()` call.
#[derive(Debug, Clone, Copy, Default)]
pub struct StartMatch;

impl reovim_driver_command::Command for StartMatch {
    fn id(&self) -> CommandId {
        ids::START_MATCH
    }

    fn description(&self) -> &'static str {
        "Start multiplayer match"
    }
}

impl CommandHandler for StartMatch {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        start_tick(runtime);
        runtime.set_mode(TetrominoMode::PLAY_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Enter the result screen (delegated from `PlayResolver` / tick).
///
/// The resolver/tick sets `screen = Result` before returning
/// `Execute(ENTER_RESULT)`. This handler performs the safe `set_mode()` call.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterResult;

impl reovim_driver_command::Command for EnterResult {
    fn id(&self) -> CommandId {
        ids::ENTER_RESULT
    }

    fn description(&self) -> &'static str {
        "Enter result screen"
    }
}

impl CommandHandler for EnterResult {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.set_mode(TetrominoMode::RESULT_ID, TransitionContext::new());
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
                let lines = state.lock_clear_spawn(&piece);
                maybe_send_garbage(runtime, lines);
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
            let lines = state.lock_clear_spawn(&dropped);
            maybe_send_garbage(runtime, lines);
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
            resume_tick(runtime);
        } else {
            state.paused = true;
            runtime.set_mode(TetrominoMode::PAUSED_ID, TransitionContext::new());
            pause_tick(runtime);
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

// ============================================================================
// Helpers
// ============================================================================

/// After locking a piece, send garbage if multiplayer and 2+ lines cleared.
#[cfg_attr(coverage_nightly, coverage(off))]
fn maybe_send_garbage(runtime: &mut SessionRuntime<'_>, lines: u32) {
    // Extract multiplayer state before shared_ext_mut
    let (is_multi, room_id) = {
        let state = runtime.ext::<TetrominoState>();
        state.map_or((false, None), |s| (s.multiplayer, s.current_room))
    };
    if lines >= 2
        && is_multi
        && let Some(room_id) = room_id
    {
        let client_id = runtime.owner().expect("client context");
        if let Some(lobby) = runtime.shared_ext_mut::<crate::state::TetrominoLobbyState>() {
            lobby.broadcast_garbage(room_id, client_id, lines - 1);
        }
    }
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(OpenMenu),
        Box::new(Start),
        Box::new(StartSingle),
        Box::new(Quit),
        Box::new(Restart),
        Box::new(EnterLobby),
        Box::new(CreateRoom),
        Box::new(ReadyToggle),
        Box::new(LeaveRoom),
        Box::new(LeaveLobby),
        Box::new(ReturnLobby),
        Box::new(MoveLeft),
        Box::new(MoveRight),
        Box::new(RotateCw),
        Box::new(RotateCcw),
        Box::new(SoftDrop),
        Box::new(HardDrop),
        Box::new(Hold),
        Box::new(Pause),
        Box::new(Tick),
        Box::new(JoinRoomByIndex),
        Box::new(StartMatch),
        Box::new(EnterResult),
    ]
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[test]
    fn open_menu_metadata() {
        let cmd = OpenMenu;
        assert_eq!(cmd.id(), ids::OPEN_MENU);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn start_metadata() {
        let cmd = Start;
        assert_eq!(cmd.id(), ids::START);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn start_single_metadata() {
        let cmd = StartSingle;
        assert_eq!(cmd.id(), ids::START_SINGLE);
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
    fn enter_lobby_metadata() {
        let cmd = EnterLobby;
        assert_eq!(cmd.id(), ids::ENTER_LOBBY);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn create_room_metadata() {
        let cmd = CreateRoom;
        assert_eq!(cmd.id(), ids::CREATE_ROOM);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn ready_toggle_metadata() {
        let cmd = ReadyToggle;
        assert_eq!(cmd.id(), ids::READY_TOGGLE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn leave_room_metadata() {
        let cmd = LeaveRoom;
        assert_eq!(cmd.id(), ids::LEAVE_ROOM);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn leave_lobby_metadata() {
        let cmd = LeaveLobby;
        assert_eq!(cmd.id(), ids::LEAVE_LOBBY);
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
    fn rotate_ccw_metadata() {
        let cmd = RotateCcw;
        assert_eq!(cmd.id(), ids::ROTATE_CCW);
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
    fn hold_metadata() {
        let cmd = Hold;
        assert_eq!(cmd.id(), ids::HOLD);
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
    fn join_room_by_index_metadata() {
        let cmd = JoinRoomByIndex;
        assert_eq!(cmd.id(), ids::JOIN_ROOM_BY_INDEX);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn start_match_metadata() {
        let cmd = StartMatch;
        assert_eq!(cmd.id(), ids::START_MATCH);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn enter_result_metadata() {
        let cmd = EnterResult;
        assert_eq!(cmd.id(), ids::ENTER_RESULT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_count() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 23);
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
        let debug = format!("{OpenMenu:?}");
        assert!(debug.contains("OpenMenu"));
        let debug = format!("{Start:?}");
        assert!(debug.contains("Start"));
        let debug = format!("{StartSingle:?}");
        assert!(debug.contains("StartSingle"));
        let debug = format!("{Quit:?}");
        assert!(debug.contains("Quit"));
        let debug = format!("{MoveLeft:?}");
        assert!(debug.contains("MoveLeft"));
        let debug = format!("{EnterLobby:?}");
        assert!(debug.contains("EnterLobby"));
        let debug = format!("{CreateRoom:?}");
        assert!(debug.contains("CreateRoom"));
        let debug = format!("{ReadyToggle:?}");
        assert!(debug.contains("ReadyToggle"));
        let debug = format!("{LeaveRoom:?}");
        assert!(debug.contains("LeaveRoom"));
        let debug = format!("{LeaveLobby:?}");
        assert!(debug.contains("LeaveLobby"));
        let debug = format!("{JoinRoomByIndex:?}");
        assert!(debug.contains("JoinRoomByIndex"));
        let debug = format!("{StartMatch:?}");
        assert!(debug.contains("StartMatch"));
        let debug = format!("{EnterResult:?}");
        assert!(debug.contains("EnterResult"));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn all_default_constructable() {
        let _ = OpenMenu::default();
        let _ = Start::default();
        let _ = StartSingle::default();
        let _ = Quit::default();
        let _ = Restart::default();
        let _ = EnterLobby::default();
        let _ = CreateRoom::default();
        let _ = ReadyToggle::default();
        let _ = LeaveRoom::default();
        let _ = LeaveLobby::default();
        let _ = MoveLeft::default();
        let _ = MoveRight::default();
        let _ = RotateCw::default();
        let _ = RotateCcw::default();
        let _ = SoftDrop::default();
        let _ = HardDrop::default();
        let _ = Hold::default();
        let _ = Pause::default();
        let _ = Tick::default();
        let _ = JoinRoomByIndex::default();
        let _ = StartMatch::default();
        let _ = EnterResult::default();
    }
}
