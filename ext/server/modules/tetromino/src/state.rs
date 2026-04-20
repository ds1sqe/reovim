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

use {rand::Rng, reovim_driver_text_session::SessionExtension};

pub use reovim_driver_text_session::ClientId;

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
#[path = "state_tests.rs"]
mod tests;
