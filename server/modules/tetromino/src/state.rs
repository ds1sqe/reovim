//! Per-client tetromino state and shared lobby state.
//!
//! `TetrominoState` is stored in the client's `ExtensionMap` as a `SessionExtension`.
//! Each connected client gets its own independent game.
//!
//! `TetrominoLobbyState` is stored in the session's shared `ExtensionMap`.
//! It manages rooms and matchmaking across all clients.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

use {rand::Rng, reovim_driver_session::SessionExtension};

pub use reovim_driver_session::ClientId;

use crate::game::{self, GameState, PieceType};

/// Maximum reroll attempts to avoid recent pieces.
const MAX_REROLLS: usize = 4;

// ============================================================================
// TetrominoScreen
// ============================================================================

/// Which screen the client is currently viewing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TetrominoScreen {
    /// Main menu (single player, multiplayer, quit).
    #[default]
    Menu,
    /// Multiplayer lobby (room list).
    Lobby,
    /// Inside a room (ready up, leave).
    Room,
    /// Countdown before match starts (3, 2, 1, GO!).
    Countdown,
    /// Active gameplay (single or multiplayer).
    Game,
    /// Match result screen (multiplayer only).
    Result,
}

/// Duration of the pre-match countdown.
pub const COUNTDOWN_DURATION: std::time::Duration = std::time::Duration::from_secs(3);

// ============================================================================
// TetrominoState (per-client)
// ============================================================================

/// Per-client tetromino state.
#[derive(Debug)]
pub struct TetrominoState {
    /// Whether the tetromino overlay is active.
    pub active: bool,
    /// Whether the game is paused.
    pub paused: bool,
    /// The current game (None if not started).
    pub game: Option<GameState>,
    /// Time of the last gravity tick.
    pub last_tick: Instant,
    /// History of recent piece types for the history-4 randomizer.
    history: VecDeque<PieceType>,
    /// Which screen the client is currently viewing.
    pub screen: TetrominoScreen,
    /// The room this client is in (if any).
    pub current_room: Option<RoomId>,
    /// Whether the current game is multiplayer.
    pub multiplayer: bool,
    /// The client's ID (set by `OpenMenu` from `runtime.owner()`).
    pub client_id: Option<ClientId>,
}

impl TetrominoState {
    /// Start a new game.
    pub fn start_game(&mut self) {
        self.history.clear();
        let first = self.next_piece();
        let next = self.next_piece();
        self.game = Some(game::new_game(first, next));
        self.active = true;
        self.paused = false;
        self.last_tick = Instant::now();
    }

    /// Get the next piece using the history-4 randomizer.
    ///
    /// Picks a random piece type. If it appears in the last 4 dealt pieces,
    /// reroll up to [`MAX_REROLLS`] times. The final pick is always accepted.
    pub fn next_piece(&mut self) -> PieceType {
        let mut rng = rand::thread_rng();
        let mut piece = PieceType::ALL[rng.gen_range(0..PieceType::ALL.len())];

        for _ in 0..MAX_REROLLS {
            if !self.history.contains(&piece) {
                break;
            }
            piece = PieceType::ALL[rng.gen_range(0..PieceType::ALL.len())];
        }

        if self.history.len() >= 4 {
            self.history.pop_front();
        }
        self.history.push_back(piece);
        piece
    }

    /// Lock the current piece, clear lines, and spawn the next.
    ///
    /// Shared by soft-drop, hard-drop, and tick logic. Returns lines cleared.
    pub fn lock_clear_spawn(&mut self, piece: &game::ActivePiece) -> u32 {
        let next = self.next_piece();
        if let Some(ref mut game_state) = self.game {
            return game::lock_and_advance(game_state, piece, next);
        }
        0
    }

    /// Check if enough time has elapsed for a gravity tick.
    #[must_use]
    pub fn should_tick(&self) -> bool {
        if self.paused {
            return false;
        }
        let Some(ref game) = self.game else {
            return false;
        };
        if game.game_over {
            return false;
        }
        let interval = std::time::Duration::from_millis(game::tick_interval_ms(game.level));
        self.last_tick.elapsed() >= interval
    }

    /// Apply a gravity tick: advance the game state and reset the timer.
    pub fn apply_tick(&mut self) {
        let next_piece = self.next_piece();
        if let Some(ref mut g) = self.game {
            game::tick(g, next_piece);
        }
        self.last_tick = Instant::now();
    }

    /// Hold the current piece: swap with held piece (or stash if empty).
    ///
    /// Returns `true` if hold was performed, `false` if already used this turn.
    ///
    /// # Panics
    ///
    /// Panics if `self.game` becomes `None` between the early-return guard
    /// and the mutable re-borrow (structurally unreachable).
    pub fn hold_piece(&mut self) -> bool {
        let Some(ref game_state) = self.game else {
            return false;
        };
        if game_state.hold_used || game_state.game_over {
            return false;
        }
        let Some(ref active) = game_state.active_piece else {
            return false;
        };

        let current_type = active.piece_type;
        let had_held = game_state.held_piece;

        // Pre-fetch next piece if we need it (first hold)
        let next_from_history = if had_held.is_none() {
            Some(self.next_piece())
        } else {
            None
        };

        let game_state = self.game.as_mut().expect("checked above");
        if let Some(held) = had_held {
            // Swap: spawn the held piece, stash the current
            game_state.active_piece = Some(game::spawn_piece(held));
            game_state.held_piece = Some(current_type);
        } else {
            // First hold: stash current, pull from next
            game_state.held_piece = Some(current_type);
            let next = game_state.next_piece;
            game_state.active_piece = Some(game::spawn_piece(next));
            game_state.next_piece = next_from_history.expect("computed above");
        }
        game_state.hold_used = true;
        true
    }
}

impl SessionExtension for TetrominoState {
    fn create() -> Self {
        Self {
            active: false,
            paused: false,
            game: None,
            last_tick: Instant::now(),
            history: VecDeque::with_capacity(4),
            screen: TetrominoScreen::Menu,
            current_room: None,
            multiplayer: false,
            client_id: None,
        }
    }
}

// ============================================================================
// Room types
// ============================================================================

/// Unique room identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(pub usize);

/// Player status in a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStatus {
    /// Joined but not ready.
    Joined,
    /// Ready to play.
    Ready,
}

/// Room status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomStatus {
    /// Waiting for players to ready up.
    Waiting,
    /// Countdown before match starts.
    Countdown,
    /// Match in progress.
    InProgress,
    /// Match finished.
    Finished,
}

/// A multiplayer room.
#[derive(Debug)]
pub struct Room {
    /// Room identifier.
    pub id: RoomId,
    /// Players in the room and their status.
    pub players: HashMap<ClientId, PlayerStatus>,
    /// Current room status.
    pub status: RoomStatus,
    /// Winner of the match (if finished).
    pub winner: Option<ClientId>,
    /// Pending garbage for each player. Key = recipient `ClientId`.
    /// Values = list of garbage line counts to apply at recipient's next tick.
    pub garbage_queue: HashMap<ClientId, Vec<u32>>,
    /// When the countdown started (set when all players ready).
    pub countdown_start: Option<Instant>,
    /// Players who have died (game over) this match.
    pub dead_players: HashSet<ClientId>,
}

impl Room {
    /// Create a new room with the given ID.
    fn new(id: RoomId) -> Self {
        Self {
            id,
            players: HashMap::new(),
            status: RoomStatus::Waiting,
            winner: None,
            garbage_queue: HashMap::new(),
            countdown_start: None,
            dead_players: HashSet::new(),
        }
    }

    /// Number of players in the room.
    #[must_use]
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
}

/// Summary of a room for lobby display.
#[derive(Debug, Clone)]
pub struct RoomSummary {
    /// Room identifier.
    pub id: RoomId,
    /// Number of players.
    pub player_count: usize,
    /// Current status.
    pub status: RoomStatus,
}

// ============================================================================
// TetrominoLobbyState (shared/session-wide)
// ============================================================================

/// Shared lobby state for multiplayer rooms.
#[derive(Debug)]
pub struct TetrominoLobbyState {
    /// Active rooms.
    pub rooms: HashMap<RoomId, Room>,
    /// Next room ID to assign.
    next_room_id: usize,
}

impl TetrominoLobbyState {
    /// Create a new room and return its ID.
    pub fn create_room(&mut self) -> RoomId {
        let id = RoomId(self.next_room_id);
        self.next_room_id += 1;
        self.rooms.insert(id, Room::new(id));
        id
    }

    /// Join an existing room. Returns `true` if the join succeeded.
    pub fn join_room(&mut self, room_id: RoomId, client_id: ClientId) -> bool {
        let Some(room) = self.rooms.get_mut(&room_id) else {
            return false;
        };
        if room.status != RoomStatus::Waiting {
            return false;
        }
        room.players.insert(client_id, PlayerStatus::Joined);
        true
    }

    /// Leave a room. Auto-removes the room if it becomes empty.
    pub fn leave_room(&mut self, room_id: RoomId, client_id: ClientId) {
        let should_remove = if let Some(room) = self.rooms.get_mut(&room_id) {
            room.players.remove(&client_id);
            room.garbage_queue.remove(&client_id);
            room.players.is_empty()
        } else {
            false
        };
        if should_remove {
            self.rooms.remove(&room_id);
        }
    }

    /// Toggle a player's ready status. Returns `Some(true)` if all players
    /// are now ready (min 2 players), `Some(false)` if not all ready,
    /// or `None` if the player/room wasn't found.
    pub fn toggle_ready(&mut self, room_id: RoomId, client_id: ClientId) -> Option<bool> {
        let room = self.rooms.get_mut(&room_id)?;
        if room.status != RoomStatus::Waiting {
            return None;
        }
        let status = room.players.get_mut(&client_id)?;
        *status = match *status {
            PlayerStatus::Joined => PlayerStatus::Ready,
            PlayerStatus::Ready => PlayerStatus::Joined,
        };
        let all_ready = room.players.len() >= 2
            && room
                .players
                .values()
                .all(|s| matches!(s, PlayerStatus::Ready));
        Some(all_ready)
    }

    /// Start a match: set room status to `InProgress`. Returns player IDs.
    pub fn start_match(&mut self, room_id: RoomId) -> Vec<ClientId> {
        let Some(room) = self.rooms.get_mut(&room_id) else {
            return Vec::new();
        };
        room.status = RoomStatus::InProgress;
        room.players.keys().copied().collect()
    }

    /// Start the pre-match countdown. Returns player IDs.
    pub fn start_countdown(&mut self, room_id: RoomId) -> Vec<ClientId> {
        let Some(room) = self.rooms.get_mut(&room_id) else {
            return Vec::new();
        };
        room.status = RoomStatus::Countdown;
        room.countdown_start = Some(Instant::now());
        room.players.keys().copied().collect()
    }

    /// Check if a room is in countdown phase.
    #[must_use]
    pub fn is_countdown(&self, room_id: RoomId) -> bool {
        self.rooms
            .get(&room_id)
            .is_some_and(|r| r.status == RoomStatus::Countdown)
    }

    /// Get seconds remaining in the countdown. Returns `None` if not counting.
    #[must_use]
    pub fn countdown_remaining(&self, room_id: RoomId) -> Option<u64> {
        let room = self.rooms.get(&room_id)?;
        let start = room.countdown_start?;
        if room.status != RoomStatus::Countdown {
            return None;
        }
        let elapsed = start.elapsed();
        if elapsed >= COUNTDOWN_DURATION {
            Some(0)
        } else {
            #[allow(clippy::cast_possible_truncation)]
            Some(COUNTDOWN_DURATION.saturating_sub(elapsed).as_secs())
        }
    }

    /// Check if a room is in progress.
    #[must_use]
    pub fn is_in_progress(&self, room_id: RoomId) -> bool {
        self.rooms
            .get(&room_id)
            .is_some_and(|r| r.status == RoomStatus::InProgress)
    }

    /// Get a sorted list of room IDs (for lobby display with digit keys).
    #[must_use]
    pub fn room_ids_sorted(&self) -> Vec<RoomId> {
        let mut ids: Vec<RoomId> = self.rooms.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Get summaries of all rooms for lobby display.
    #[must_use]
    pub fn room_summaries(&self) -> Vec<RoomSummary> {
        let mut summaries: Vec<RoomSummary> = self
            .rooms
            .values()
            .map(|r| RoomSummary {
                id: r.id,
                player_count: r.player_count(),
                status: r.status,
            })
            .collect();
        summaries.sort_by_key(|s| s.id.0);
        summaries
    }

    /// Broadcast garbage lines to all opponents in a room (except the sender).
    pub fn broadcast_garbage(&mut self, room_id: RoomId, sender: ClientId, line_count: u32) {
        let Some(room) = self.rooms.get_mut(&room_id) else {
            return;
        };
        let opponents: Vec<ClientId> = room
            .players
            .keys()
            .filter(|&&id| id != sender)
            .copied()
            .collect();
        for opp in opponents {
            room.garbage_queue.entry(opp).or_default().push(line_count);
        }
    }

    /// Take all pending garbage for a player (drains the queue).
    pub fn take_garbage(&mut self, room_id: RoomId, client_id: ClientId) -> Vec<u32> {
        let Some(room) = self.rooms.get_mut(&room_id) else {
            return Vec::new();
        };
        room.garbage_queue.remove(&client_id).unwrap_or_default()
    }

    /// Mark a player as dead (game over) in their room.
    pub fn mark_dead(&mut self, room_id: RoomId, client_id: ClientId) {
        if let Some(room) = self.rooms.get_mut(&room_id) {
            room.dead_players.insert(client_id);
        }
    }

    /// Count alive players (not dead) in a room.
    #[must_use]
    pub fn alive_count(&self, room_id: RoomId) -> usize {
        self.rooms
            .get(&room_id)
            .map_or(0, |r| r.players.len() - r.dead_players.len())
    }

    /// Finish a match: set room status to `Finished` and record the winner.
    pub fn finish_match(&mut self, room_id: RoomId, winner: ClientId) {
        if let Some(room) = self.rooms.get_mut(&room_id) {
            room.status = RoomStatus::Finished;
            room.winner = Some(winner);
        }
    }

    /// Get the last alive player in a room (the winner), if exactly one remains.
    #[must_use]
    pub fn last_alive(&self, room_id: RoomId) -> Option<ClientId> {
        self.rooms.get(&room_id).and_then(|r| {
            r.players
                .keys()
                .find(|id| !r.dead_players.contains(id))
                .copied()
        })
    }

    /// Check if a room's match is finished.
    #[must_use]
    pub fn is_finished(&self, room_id: RoomId) -> bool {
        self.rooms
            .get(&room_id)
            .is_some_and(|r| r.status == RoomStatus::Finished)
    }
}

impl SessionExtension for TetrominoLobbyState {
    fn create() -> Self {
        Self {
            rooms: HashMap::new(),
            next_room_id: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TetrominoScreen
    // =========================================================================

    #[test]
    fn screen_default_is_menu() {
        assert_eq!(TetrominoScreen::default(), TetrominoScreen::Menu);
    }

    #[test]
    fn screen_debug() {
        let debug = format!("{:?}", TetrominoScreen::Game);
        assert!(debug.contains("Game"));
    }

    #[test]
    fn screen_copy_eq() {
        let a = TetrominoScreen::Lobby;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn screen_ne() {
        assert_ne!(TetrominoScreen::Menu, TetrominoScreen::Game);
    }

    // =========================================================================
    // TetrominoState
    // =========================================================================

    #[test]
    fn state_create_defaults() {
        let state = TetrominoState::create();
        assert!(!state.active);
        assert!(!state.paused);
        assert!(state.game.is_none());
        assert!(state.history.is_empty());
        assert_eq!(state.screen, TetrominoScreen::Menu);
        assert!(state.current_room.is_none());
        assert!(!state.multiplayer);
        assert!(state.client_id.is_none());
    }

    #[test]
    fn state_debug() {
        let state = TetrominoState::create();
        let debug = format!("{state:?}");
        assert!(debug.contains("TetrominoState"));
    }

    #[test]
    fn start_game_activates() {
        let mut state = TetrominoState::create();
        state.start_game();
        assert!(state.active);
        assert!(!state.paused);
        assert!(state.game.is_some());
        assert!(!state.game.as_ref().unwrap().game_over);
    }

    #[test]
    fn start_game_populates_history() {
        let mut state = TetrominoState::create();
        state.start_game();
        // start_game calls next_piece twice (first + next)
        assert_eq!(state.history.len(), 2);
    }

    #[test]
    fn start_game_clears_old_history() {
        let mut state = TetrominoState::create();
        state.start_game();
        assert_eq!(state.history.len(), 2);
        // Start a new game — history should be reset
        state.start_game();
        assert_eq!(state.history.len(), 2);
    }

    #[test]
    fn next_piece_returns_valid_type() {
        let mut state = TetrominoState::create();
        for _ in 0..50 {
            let piece = state.next_piece();
            assert!(PieceType::ALL.contains(&piece));
        }
    }

    #[test]
    fn next_piece_history_caps_at_4() {
        let mut state = TetrominoState::create();
        for _ in 0..20 {
            state.next_piece();
        }
        assert_eq!(state.history.len(), 4);
    }

    #[test]
    fn next_piece_all_types_appear() {
        // Over many draws, all 10 types should appear
        let mut state = TetrominoState::create();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            seen.insert(state.next_piece());
        }
        assert_eq!(seen.len(), 10);
    }

    #[test]
    fn should_tick_false_when_paused() {
        let mut state = TetrominoState::create();
        state.start_game();
        state.paused = true;
        assert!(!state.should_tick());
    }

    #[test]
    fn should_tick_false_when_no_game() {
        let state = TetrominoState::create();
        assert!(!state.should_tick());
    }

    #[test]
    fn should_tick_false_when_game_over() {
        let mut state = TetrominoState::create();
        state.start_game();
        state.game.as_mut().unwrap().game_over = true;
        assert!(!state.should_tick());
    }

    #[test]
    fn should_tick_false_immediately() {
        let mut state = TetrominoState::create();
        state.start_game();
        // Just started, should not tick immediately (900ms interval at level 0)
        assert!(!state.should_tick());
    }

    #[test]
    fn apply_tick_advances_game() {
        let mut state = TetrominoState::create();
        state.start_game();
        let orig_row = state
            .game
            .as_ref()
            .unwrap()
            .active_piece
            .as_ref()
            .unwrap()
            .row;
        state.apply_tick();
        let new_row = state
            .game
            .as_ref()
            .unwrap()
            .active_piece
            .as_ref()
            .unwrap()
            .row;
        assert_eq!(new_row, orig_row + 1);
    }

    #[test]
    fn apply_tick_no_game_does_nothing() {
        let mut state = TetrominoState::create();
        state.apply_tick(); // Should not panic
    }

    // =========================================================================
    // lock_clear_spawn
    // =========================================================================

    #[test]
    fn lock_clear_spawn_advances_game() {
        let mut state = TetrominoState::create();
        state.start_game();
        state.game.as_mut().unwrap().hold_used = true;
        let piece = state.game.as_ref().unwrap().active_piece.clone().unwrap();
        state.lock_clear_spawn(&piece);
        // hold_used should be reset
        assert!(!state.game.as_ref().unwrap().hold_used);
    }

    #[test]
    fn lock_clear_spawn_no_game() {
        let mut state = TetrominoState::create();
        let piece = game::spawn_piece(PieceType::Tee);
        assert_eq!(state.lock_clear_spawn(&piece), 0);
    }

    // =========================================================================
    // hold_piece
    // =========================================================================

    #[test]
    fn hold_piece_first_time() {
        let mut state = TetrominoState::create();
        state.start_game();
        let current = state
            .game
            .as_ref()
            .unwrap()
            .active_piece
            .as_ref()
            .unwrap()
            .piece_type;
        assert!(state.hold_piece());
        assert_eq!(state.game.as_ref().unwrap().held_piece, Some(current));
        assert!(state.game.as_ref().unwrap().hold_used);
    }

    #[test]
    fn hold_piece_swap() {
        let mut state = TetrominoState::create();
        state.start_game();
        // First hold
        state.hold_piece();
        let held = state.game.as_ref().unwrap().held_piece.unwrap();
        // Reset hold_used to allow another hold
        state.game.as_mut().unwrap().hold_used = false;
        let current = state
            .game
            .as_ref()
            .unwrap()
            .active_piece
            .as_ref()
            .unwrap()
            .piece_type;
        // Second hold — should swap
        assert!(state.hold_piece());
        assert_eq!(state.game.as_ref().unwrap().held_piece, Some(current));
        assert_eq!(
            state
                .game
                .as_ref()
                .unwrap()
                .active_piece
                .as_ref()
                .unwrap()
                .piece_type,
            held
        );
    }

    #[test]
    fn hold_piece_blocked_when_already_used() {
        let mut state = TetrominoState::create();
        state.start_game();
        assert!(state.hold_piece());
        // Second attempt should fail (hold_used = true)
        assert!(!state.hold_piece());
    }

    #[test]
    fn hold_piece_no_game() {
        let mut state = TetrominoState::create();
        assert!(!state.hold_piece());
    }

    #[test]
    fn hold_piece_game_over() {
        let mut state = TetrominoState::create();
        state.start_game();
        state.game.as_mut().unwrap().game_over = true;
        assert!(!state.hold_piece());
    }

    #[test]
    fn hold_piece_no_active_piece() {
        let mut state = TetrominoState::create();
        state.start_game();
        state.game.as_mut().unwrap().active_piece = None;
        assert!(!state.hold_piece());
    }

    // =========================================================================
    // RoomId
    // =========================================================================

    #[test]
    fn room_id_copy_eq() {
        let a = RoomId(1);
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn room_id_ne() {
        assert_ne!(RoomId(1), RoomId(2));
    }

    #[test]
    fn room_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(RoomId(1));
        set.insert(RoomId(2));
        assert!(set.contains(&RoomId(1)));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn room_id_debug() {
        let debug = format!("{:?}", RoomId(42));
        assert!(debug.contains("42"));
    }

    // =========================================================================
    // PlayerStatus
    // =========================================================================

    #[test]
    fn player_status_debug() {
        let debug = format!("{:?}", PlayerStatus::Ready);
        assert!(debug.contains("Ready"));
    }

    #[test]
    fn player_status_eq() {
        assert_eq!(PlayerStatus::Joined, PlayerStatus::Joined);
        assert_ne!(PlayerStatus::Joined, PlayerStatus::Ready);
    }

    #[test]
    fn player_status_copy() {
        let a = PlayerStatus::Ready;
        let b = a;
        assert_eq!(a, b);
    }

    // =========================================================================
    // RoomStatus
    // =========================================================================

    #[test]
    fn room_status_debug() {
        let debug = format!("{:?}", RoomStatus::InProgress);
        assert!(debug.contains("InProgress"));
    }

    #[test]
    fn room_status_eq() {
        assert_eq!(RoomStatus::Waiting, RoomStatus::Waiting);
        assert_ne!(RoomStatus::Waiting, RoomStatus::InProgress);
    }

    #[test]
    fn room_status_copy() {
        let a = RoomStatus::Finished;
        let b = a;
        assert_eq!(a, b);
    }

    // =========================================================================
    // Room
    // =========================================================================

    #[test]
    fn room_new() {
        let room = Room::new(RoomId(1));
        assert_eq!(room.id, RoomId(1));
        assert!(room.players.is_empty());
        assert_eq!(room.status, RoomStatus::Waiting);
        assert!(room.winner.is_none());
        assert!(room.garbage_queue.is_empty());
    }

    #[test]
    fn room_player_count() {
        let mut room = Room::new(RoomId(1));
        assert_eq!(room.player_count(), 0);
        room.players.insert(ClientId(1), PlayerStatus::Joined);
        assert_eq!(room.player_count(), 1);
    }

    #[test]
    fn room_debug() {
        let room = Room::new(RoomId(1));
        let debug = format!("{room:?}");
        assert!(debug.contains("Room"));
    }

    // =========================================================================
    // RoomSummary
    // =========================================================================

    #[test]
    fn room_summary_debug() {
        let summary = RoomSummary {
            id: RoomId(1),
            player_count: 2,
            status: RoomStatus::Waiting,
        };
        let debug = format!("{summary:?}");
        assert!(debug.contains("RoomSummary"));
    }

    #[test]
    fn room_summary_clone() {
        let summary = RoomSummary {
            id: RoomId(1),
            player_count: 2,
            status: RoomStatus::Waiting,
        };
        let cloned = Clone::clone(&summary);
        assert_eq!(cloned.id, RoomId(1));
        assert_eq!(cloned.player_count, 2);
    }

    // =========================================================================
    // TetrominoLobbyState
    // =========================================================================

    #[test]
    fn lobby_create() {
        let lobby = TetrominoLobbyState::create();
        assert!(lobby.rooms.is_empty());
    }

    #[test]
    fn lobby_debug() {
        let lobby = TetrominoLobbyState::create();
        let debug = format!("{lobby:?}");
        assert!(debug.contains("TetrominoLobbyState"));
    }

    #[test]
    fn lobby_create_room() {
        let mut lobby = TetrominoLobbyState::create();
        let id1 = lobby.create_room();
        let id2 = lobby.create_room();
        assert_ne!(id1, id2);
        assert_eq!(lobby.rooms.len(), 2);
    }

    #[test]
    fn lobby_join_room() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        assert!(lobby.join_room(room_id, ClientId(1)));
        assert_eq!(lobby.rooms[&room_id].player_count(), 1);
    }

    #[test]
    fn lobby_join_nonexistent_room() {
        let mut lobby = TetrominoLobbyState::create();
        assert!(!lobby.join_room(RoomId(999), ClientId(1)));
    }

    #[test]
    fn lobby_join_in_progress_room() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.rooms.get_mut(&room_id).unwrap().status = RoomStatus::InProgress;
        assert!(!lobby.join_room(room_id, ClientId(2)));
    }

    #[test]
    fn lobby_leave_room() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.leave_room(room_id, ClientId(1));
        assert_eq!(lobby.rooms[&room_id].player_count(), 1);
    }

    #[test]
    fn lobby_leave_room_auto_removes_empty() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.leave_room(room_id, ClientId(1));
        assert!(!lobby.rooms.contains_key(&room_id));
    }

    #[test]
    fn lobby_leave_nonexistent_room() {
        let mut lobby = TetrominoLobbyState::create();
        lobby.leave_room(RoomId(999), ClientId(1)); // should not panic
    }

    #[test]
    fn lobby_toggle_ready() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        let result = lobby.toggle_ready(room_id, ClientId(1));
        assert_eq!(result, Some(false)); // only 1 player, can't be all ready
        assert_eq!(lobby.rooms[&room_id].players[&ClientId(1)], PlayerStatus::Ready);
    }

    #[test]
    fn lobby_toggle_ready_back_to_joined() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.toggle_ready(room_id, ClientId(1)); // -> Ready
        lobby.toggle_ready(room_id, ClientId(1)); // -> Joined
        assert_eq!(lobby.rooms[&room_id].players[&ClientId(1)], PlayerStatus::Joined);
    }

    #[test]
    fn lobby_toggle_ready_all_ready_min_2() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.toggle_ready(room_id, ClientId(1));
        let result = lobby.toggle_ready(room_id, ClientId(2));
        assert_eq!(result, Some(true)); // all 2 players ready
    }

    #[test]
    fn lobby_toggle_ready_nonexistent() {
        let mut lobby = TetrominoLobbyState::create();
        assert!(lobby.toggle_ready(RoomId(999), ClientId(1)).is_none());
    }

    #[test]
    fn lobby_toggle_ready_nonexistent_player() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        assert!(lobby.toggle_ready(room_id, ClientId(99)).is_none());
    }

    #[test]
    fn lobby_toggle_ready_in_progress() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.rooms.get_mut(&room_id).unwrap().status = RoomStatus::InProgress;
        assert!(lobby.toggle_ready(room_id, ClientId(1)).is_none());
    }

    #[test]
    fn lobby_start_match() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        let players = lobby.start_match(room_id);
        assert_eq!(players.len(), 2);
        assert_eq!(lobby.rooms[&room_id].status, RoomStatus::InProgress);
    }

    #[test]
    fn lobby_start_match_nonexistent() {
        let mut lobby = TetrominoLobbyState::create();
        assert!(lobby.start_match(RoomId(999)).is_empty());
    }

    #[test]
    fn lobby_is_in_progress() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        assert!(!lobby.is_in_progress(room_id));
        lobby.start_match(room_id);
        assert!(lobby.is_in_progress(room_id));
    }

    #[test]
    fn lobby_is_in_progress_nonexistent() {
        let lobby = TetrominoLobbyState::create();
        assert!(!lobby.is_in_progress(RoomId(999)));
    }

    #[test]
    fn lobby_room_ids_sorted() {
        let mut lobby = TetrominoLobbyState::create();
        let _id1 = lobby.create_room();
        let _id2 = lobby.create_room();
        let _id3 = lobby.create_room();
        let ids = lobby.room_ids_sorted();
        assert_eq!(ids.len(), 3);
        assert!(ids[0].0 < ids[1].0);
        assert!(ids[1].0 < ids[2].0);
    }

    #[test]
    fn lobby_room_summaries() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        let summaries = lobby.room_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].player_count, 1);
        assert_eq!(summaries[0].status, RoomStatus::Waiting);
    }

    #[test]
    fn lobby_broadcast_garbage() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.join_room(room_id, ClientId(3));
        lobby.broadcast_garbage(room_id, ClientId(1), 2);
        // Client 2 and 3 should have garbage, client 1 should not
        let g2 = lobby.take_garbage(room_id, ClientId(2));
        assert_eq!(g2, vec![2]);
        let g3 = lobby.take_garbage(room_id, ClientId(3));
        assert_eq!(g3, vec![2]);
        let g1 = lobby.take_garbage(room_id, ClientId(1));
        assert!(g1.is_empty());
    }

    #[test]
    fn lobby_broadcast_garbage_nonexistent_room() {
        let mut lobby = TetrominoLobbyState::create();
        lobby.broadcast_garbage(RoomId(999), ClientId(1), 2); // should not panic
    }

    #[test]
    fn lobby_take_garbage_empty() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        assert!(lobby.take_garbage(room_id, ClientId(1)).is_empty());
    }

    #[test]
    fn lobby_take_garbage_nonexistent_room() {
        let mut lobby = TetrominoLobbyState::create();
        assert!(lobby.take_garbage(RoomId(999), ClientId(1)).is_empty());
    }

    #[test]
    fn lobby_take_garbage_drains() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.broadcast_garbage(room_id, ClientId(1), 3);
        let g = lobby.take_garbage(room_id, ClientId(2));
        assert_eq!(g, vec![3]);
        // Second take should be empty
        let g2 = lobby.take_garbage(room_id, ClientId(2));
        assert!(g2.is_empty());
    }

    #[test]
    fn lobby_leave_room_cleans_garbage_queue() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.broadcast_garbage(room_id, ClientId(1), 2);
        lobby.leave_room(room_id, ClientId(2));
        // Room still exists (client 1 is still in)
        assert!(lobby.rooms.contains_key(&room_id));
        // Client 2's garbage should be cleaned
        assert!(
            !lobby.rooms[&room_id]
                .garbage_queue
                .contains_key(&ClientId(2))
        );
    }

    // =========================================================================
    // Countdown (#544)
    // =========================================================================

    #[test]
    fn lobby_start_countdown() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        let players = lobby.start_countdown(room_id);
        assert_eq!(players.len(), 2);
        assert_eq!(lobby.rooms[&room_id].status, RoomStatus::Countdown);
        assert!(lobby.rooms[&room_id].countdown_start.is_some());
    }

    #[test]
    fn lobby_start_countdown_nonexistent() {
        let mut lobby = TetrominoLobbyState::create();
        assert!(lobby.start_countdown(RoomId(999)).is_empty());
    }

    #[test]
    fn lobby_is_countdown() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        assert!(!lobby.is_countdown(room_id));
        lobby.start_countdown(room_id);
        assert!(lobby.is_countdown(room_id));
    }

    #[test]
    fn lobby_is_countdown_nonexistent() {
        let lobby = TetrominoLobbyState::create();
        assert!(!lobby.is_countdown(RoomId(999)));
    }

    #[test]
    fn lobby_countdown_remaining_not_counting() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        // Room is Waiting, not Countdown
        assert!(lobby.countdown_remaining(room_id).is_none());
    }

    #[test]
    fn lobby_countdown_remaining_nonexistent() {
        let lobby = TetrominoLobbyState::create();
        assert!(lobby.countdown_remaining(RoomId(999)).is_none());
    }

    #[test]
    fn lobby_countdown_remaining_active() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.start_countdown(room_id);
        // Just started — should have ~3 seconds remaining
        let remaining = lobby.countdown_remaining(room_id).unwrap();
        assert!(remaining <= 3);
    }

    #[test]
    fn lobby_countdown_remaining_expired() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.start_countdown(room_id);
        // Backdate the start to simulate expiry
        let room = lobby.rooms.get_mut(&room_id).unwrap();
        room.countdown_start = Some(
            Instant::now()
                .checked_sub(std::time::Duration::from_secs(5))
                .unwrap(),
        );
        assert_eq!(lobby.countdown_remaining(room_id), Some(0));
    }

    #[test]
    fn lobby_countdown_remaining_no_start_time() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        // Set status to Countdown but no start time
        lobby.rooms.get_mut(&room_id).unwrap().status = RoomStatus::Countdown;
        assert!(lobby.countdown_remaining(room_id).is_none());
    }

    #[test]
    fn room_countdown_start_default_none() {
        let room = Room::new(RoomId(1));
        assert!(room.countdown_start.is_none());
    }

    #[test]
    fn room_status_countdown_eq() {
        assert_eq!(RoomStatus::Countdown, RoomStatus::Countdown);
        assert_ne!(RoomStatus::Countdown, RoomStatus::Waiting);
    }

    #[test]
    fn screen_countdown_eq() {
        assert_eq!(TetrominoScreen::Countdown, TetrominoScreen::Countdown);
        assert_ne!(TetrominoScreen::Countdown, TetrominoScreen::Room);
    }

    // =========================================================================
    // Result screen
    // =========================================================================

    #[test]
    fn screen_result_eq() {
        assert_eq!(TetrominoScreen::Result, TetrominoScreen::Result);
        assert_ne!(TetrominoScreen::Result, TetrominoScreen::Game);
    }

    #[test]
    fn screen_result_debug() {
        let debug = format!("{:?}", TetrominoScreen::Result);
        assert!(debug.contains("Result"));
    }

    // =========================================================================
    // mark_dead / alive_count / finish_match / last_alive / is_finished
    // =========================================================================

    #[test]
    fn lobby_mark_dead() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.mark_dead(room_id, ClientId(1));
        assert!(lobby.rooms[&room_id].dead_players.contains(&ClientId(1)));
        assert!(!lobby.rooms[&room_id].dead_players.contains(&ClientId(2)));
    }

    #[test]
    fn lobby_mark_dead_nonexistent_room() {
        let mut lobby = TetrominoLobbyState::create();
        lobby.mark_dead(RoomId(999), ClientId(1)); // should not panic
    }

    #[test]
    fn lobby_alive_count() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.join_room(room_id, ClientId(3));
        assert_eq!(lobby.alive_count(room_id), 3);
        lobby.mark_dead(room_id, ClientId(1));
        assert_eq!(lobby.alive_count(room_id), 2);
        lobby.mark_dead(room_id, ClientId(2));
        assert_eq!(lobby.alive_count(room_id), 1);
    }

    #[test]
    fn lobby_alive_count_nonexistent() {
        let lobby = TetrominoLobbyState::create();
        assert_eq!(lobby.alive_count(RoomId(999)), 0);
    }

    #[test]
    fn lobby_finish_match() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.start_match(room_id);
        lobby.finish_match(room_id, ClientId(2));
        assert_eq!(lobby.rooms[&room_id].status, RoomStatus::Finished);
        assert_eq!(lobby.rooms[&room_id].winner, Some(ClientId(2)));
    }

    #[test]
    fn lobby_finish_match_nonexistent() {
        let mut lobby = TetrominoLobbyState::create();
        lobby.finish_match(RoomId(999), ClientId(1)); // should not panic
    }

    #[test]
    fn lobby_last_alive() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.mark_dead(room_id, ClientId(1));
        assert_eq!(lobby.last_alive(room_id), Some(ClientId(2)));
    }

    #[test]
    fn lobby_last_alive_all_dead() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.mark_dead(room_id, ClientId(1));
        assert!(lobby.last_alive(room_id).is_none());
    }

    #[test]
    fn lobby_last_alive_nonexistent() {
        let lobby = TetrominoLobbyState::create();
        assert!(lobby.last_alive(RoomId(999)).is_none());
    }

    #[test]
    fn lobby_is_finished() {
        let mut lobby = TetrominoLobbyState::create();
        let room_id = lobby.create_room();
        assert!(!lobby.is_finished(room_id));
        lobby.finish_match(room_id, ClientId(1));
        assert!(lobby.is_finished(room_id));
    }

    #[test]
    fn lobby_is_finished_nonexistent() {
        let lobby = TetrominoLobbyState::create();
        assert!(!lobby.is_finished(RoomId(999)));
    }

    #[test]
    fn room_dead_players_default_empty() {
        let room = Room::new(RoomId(1));
        assert!(room.dead_players.is_empty());
    }

    #[test]
    fn room_status_finished_eq() {
        assert_eq!(RoomStatus::Finished, RoomStatus::Finished);
        assert_ne!(RoomStatus::Finished, RoomStatus::InProgress);
    }
}
