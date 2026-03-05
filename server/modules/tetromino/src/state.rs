//! Per-client tetromino state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Each connected client gets its own independent game.

use std::{collections::VecDeque, time::Instant};

use {rand::Rng, reovim_driver_session::SessionExtension};

use crate::game::{self, GameState, PieceType};

/// Maximum reroll attempts to avoid recent pieces.
const MAX_REROLLS: usize = 4;

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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_create_defaults() {
        let state = TetrominoState::create();
        assert!(!state.active);
        assert!(!state.paused);
        assert!(state.game.is_none());
        assert!(state.history.is_empty());
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
        // Over many draws, all 7 types should appear
        let mut state = TetrominoState::create();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            seen.insert(state.next_piece());
        }
        assert_eq!(seen.len(), 7);
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
        let piece = game::spawn_piece(PieceType::T);
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
}
