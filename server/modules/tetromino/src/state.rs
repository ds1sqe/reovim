//! Per-client tetromino state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Each connected client gets its own independent game.

use std::time::Instant;

use {rand::seq::SliceRandom, reovim_driver_session::SessionExtension};

use crate::game::{self, GameState, PieceType};

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
    /// Piece bag for pseudo-random piece generation.
    bag: Vec<PieceType>,
    /// Current index into the bag.
    bag_index: usize,
}

impl TetrominoState {
    /// Start a new game.
    pub fn start_game(&mut self) {
        let first = self.next_piece_from_bag();
        let next = self.next_piece_from_bag();
        self.game = Some(game::new_game(first, next));
        self.active = true;
        self.paused = false;
        self.last_tick = Instant::now();
    }

    /// Get the next piece from the 7-bag randomizer.
    ///
    /// Cycles through all 7 piece types. When a bag is exhausted,
    /// the bag is reshuffled using Fisher-Yates (via `rand`).
    pub fn next_piece_from_bag(&mut self) -> PieceType {
        if self.bag_index >= self.bag.len() {
            self.bag.shuffle(&mut rand::thread_rng());
            self.bag_index = 0;
        }
        let piece = self.bag[self.bag_index];
        self.bag_index += 1;
        piece
    }

    /// Lock the current piece, clear lines, and spawn the next.
    ///
    /// Shared by soft-drop, hard-drop, and tick logic. Returns lines cleared.
    pub fn lock_clear_spawn(&mut self, piece: &game::ActivePiece) -> u32 {
        let next = self.next_piece_from_bag();
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
        let next_piece = self.next_piece_from_bag();
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

        // Pre-fetch next piece from bag if we need it (first hold)
        let next_from_bag = if had_held.is_none() {
            Some(self.next_piece_from_bag())
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
            game_state.next_piece = next_from_bag.expect("computed above");
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
            bag: vec![
                PieceType::I,
                PieceType::O,
                PieceType::T,
                PieceType::S,
                PieceType::Z,
                PieceType::J,
                PieceType::L,
            ],
            bag_index: 0,
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
        assert_eq!(state.bag.len(), 7);
        assert_eq!(state.bag_index, 0);
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
    fn start_game_uses_bag() {
        let mut state = TetrominoState::create();
        state.start_game();
        // First piece from bag is I, second is O
        assert_eq!(
            state
                .game
                .as_ref()
                .unwrap()
                .active_piece
                .as_ref()
                .unwrap()
                .piece_type,
            PieceType::I
        );
        assert_eq!(state.game.as_ref().unwrap().next_piece, PieceType::O);
        assert_eq!(state.bag_index, 2);
    }

    #[test]
    fn next_piece_all_types_in_bag() {
        let mut state = TetrominoState::create();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..7 {
            seen.insert(state.next_piece_from_bag());
        }
        assert_eq!(seen.len(), 7);
    }

    #[test]
    fn next_piece_reshuffles_bag() {
        let mut state = TetrominoState::create();
        // Exhaust first bag
        for _ in 0..7 {
            state.next_piece_from_bag();
        }
        // Second bag should still have all 7 types (just different order)
        let mut seen = std::collections::HashSet::new();
        for _ in 0..7 {
            seen.insert(state.next_piece_from_bag());
        }
        assert_eq!(seen.len(), 7);
    }

    #[test]
    fn should_tick_false_when_paused() {
        let mut state = TetrominoState::create();
        state.start_game();
        state.paused = true;
        // Even with time elapsed, should not tick when paused
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
        // Just started, should not tick immediately (1000ms interval at level 0)
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
