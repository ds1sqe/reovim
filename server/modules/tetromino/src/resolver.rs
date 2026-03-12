//! Tetromino mode key resolvers.
//!
//! Five resolvers for the five tetromino modes:
//! - `PlayResolver` for `tetromino:PLAY` (game keys via keymap, tick check)
//! - `PausedResolver` for `tetromino:PAUSED` (only unpause/quit via keymap)
//! - `MenuResolver` for `tetromino:MENU` (keymap only)
//! - `LobbyResolver` for `tetromino:LOBBY` (digit keys for room join + keymap)
//! - `RoomResolver` for `tetromino:ROOM` (match-start detection + keymap)

use std::sync::RwLock;

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, ResolveContext,
        ResolveInput, ResolveResult,
    },
    reovim_driver_session::ExtensionMap,
    reovim_kernel::api::v1::ModeId,
};

use crate::{
    modes::TetrominoMode,
    state::{TetrominoLobbyState, TetrominoScreen, TetrominoState},
};

// ============================================================================
// PlayResolver
// ============================================================================

/// Key resolver for tetromino play mode.
///
/// On every key event, first checks if a gravity tick is due (resolver-driven
/// tick), then resolves the key via the keymap.
pub struct PlayResolver {
    mode_id: ModeId,
    pending_keys: RwLock<KeySequence>,
}

impl PlayResolver {
    /// Create a new play mode resolver.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new is not const-stable
    pub fn new() -> Self {
        Self {
            mode_id: TetrominoMode::PLAY_ID,
            pending_keys: RwLock::new(KeySequence::new()),
        }
    }
}

impl Default for PlayResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for PlayResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        let game_state = client_extensions.get_or_insert::<TetrominoState>();

        // If the match finished (detected by tick), delegate to command handler
        // for safe set_mode() (avoids ModeTransition::Set stack corruption).
        if game_state.screen == TetrominoScreen::Result {
            return ResolveResult::Execute(crate::ids::ENTER_RESULT, ResolveContext::new());
        }

        // Multiplayer: apply pending garbage from opponents
        if game_state.multiplayer
            && let Some(room_id) = game_state.current_room
            && let Some(client_id) = game_state.client_id
            && let Some(lobby) = shared_extensions.get_mut::<TetrominoLobbyState>()
        {
            let garbage = lobby.take_garbage(room_id, client_id);
            if let Some(ref mut game) = game_state.game {
                let mut rng = rand::thread_rng();
                for count in garbage {
                    for _ in 0..count {
                        let gap = rand::Rng::gen_range(&mut rng, 0..crate::game::BOARD_WIDTH);
                        crate::game::apply_garbage_line(game, gap);
                    }
                }
            }
        }

        // Resolver-driven tick: check if gravity should advance
        let game_state = client_extensions.get_or_insert::<TetrominoState>();
        if game_state.should_tick() {
            game_state.apply_tick();
        }

        // Then resolve the key through the keymap
        self.resolve_with_keymap(key, state, input)
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        self.pending_keys.write().expect("lock poisoned").push(*key);
        let keys = self.pending_keys.read().expect("lock poisoned").clone();

        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                self.pending_keys.write().expect("lock poisoned").clear();
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => {
                self.pending_keys.write().expect("lock poisoned").clear();
                ResolveResult::NotHandled
            }
        }
    }

    fn reset(&mut self) {
        self.pending_keys.write().expect("lock poisoned").clear();
    }

    fn pending_keys(&self) -> KeySequence {
        self.pending_keys.read().expect("lock poisoned").clone()
    }
}

// ============================================================================
// PausedResolver
// ============================================================================

/// Key resolver for tetromino paused mode.
///
/// Only resolves keys through the keymap (p to unpause, q/Esc to quit).
/// Does NOT check ticks since the game is paused.
pub struct PausedResolver {
    mode_id: ModeId,
}

impl PausedResolver {
    /// Create a new paused mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: TetrominoMode::PAUSED_ID,
        }
    }
}

impl Default for PausedResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for PausedResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }
}

// ============================================================================
// MenuResolver
// ============================================================================

/// Key resolver for tetromino menu mode.
///
/// Keymap-only resolver (s, m, q/Esc).
pub struct MenuResolver {
    mode_id: ModeId,
}

impl MenuResolver {
    /// Create a new menu mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: TetrominoMode::MENU_ID,
        }
    }
}

impl Default for MenuResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for MenuResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }
}

// ============================================================================
// LobbyResolver
// ============================================================================

/// Key resolver for tetromino lobby mode.
///
/// Handles digit keys (1-9) for room joining directly via extensions,
/// and delegates other keys to the keymap (c, q/Esc).
pub struct LobbyResolver {
    mode_id: ModeId,
}

impl LobbyResolver {
    /// Create a new lobby mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: TetrominoMode::LOBBY_ID,
        }
    }
}

impl Default for LobbyResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for LobbyResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Check for digit key -> join room directly
        if let KeyCode::Char(c @ '1'..='9') = key.code {
            let index = (c as usize) - ('1' as usize);
            if let Some(lobby) = shared_extensions.get_mut::<TetrominoLobbyState>() {
                let rooms = lobby.room_ids_sorted();
                if index < rooms.len() {
                    let room_id = rooms[index];
                    let tetro = client_extensions.get_or_insert::<TetrominoState>();
                    let Some(client_id) = tetro.client_id else {
                        return ResolveResult::NotHandled;
                    };
                    if lobby.join_room(room_id, client_id) {
                        tetro.current_room = Some(room_id);
                        tetro.screen = TetrominoScreen::Room;
                        return ResolveResult::Execute(
                            crate::ids::JOIN_ROOM_BY_INDEX,
                            ResolveContext::new(),
                        );
                    }
                }
            }
            return ResolveResult::NotHandled;
        }

        // Non-digit keys -> delegate to keymap
        self.resolve_with_keymap(key, state, input)
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }
}

// ============================================================================
// RoomResolver
// ============================================================================

/// Key resolver for tetromino room mode.
///
/// Detects when a match starts (room transitions to `InProgress` by another
/// player's ready toggle) and transitions to PLAY mode. Otherwise delegates
/// to keymap (r, q/Esc).
pub struct RoomResolver {
    mode_id: ModeId,
}

impl RoomResolver {
    /// Create a new room mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: TetrominoMode::ROOM_ID,
        }
    }
}

impl Default for RoomResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for RoomResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        let tetro = client_extensions.get_or_insert::<TetrominoState>();
        if let Some(room_id) = tetro.current_room
            && let Some(lobby) = shared_extensions.get::<TetrominoLobbyState>()
        {
            // Detect countdown started by another player's ready toggle
            if lobby.is_countdown(room_id) && tetro.screen == TetrominoScreen::Room {
                tetro.screen = TetrominoScreen::Countdown;
                // Stay in ROOM mode — tick will handle transition to game
            }

            // Match started (countdown finished) → transition to PLAY.
            // The tick catch-up may have already set screen=Game and started
            // the game, so also accept screen==Game to trigger the mode change.
            if lobby.is_in_progress(room_id) {
                if tetro.screen == TetrominoScreen::Room
                    || tetro.screen == TetrominoScreen::Countdown
                {
                    // Tick hasn't caught up yet — initialize game
                    tetro.screen = TetrominoScreen::Game;
                    tetro.multiplayer = true;
                    tetro.start_game();
                }
                if tetro.screen == TetrominoScreen::Game {
                    return ResolveResult::Execute(crate::ids::START_MATCH, ResolveContext::new());
                }
            }
        }
        self.resolve_with_keymap(key, state, input)
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }
}

// ============================================================================
// ResultResolver
// ============================================================================

/// Key resolver for tetromino result mode.
///
/// Keymap-only resolver (q/Esc to return to lobby).
pub struct ResultResolver {
    mode_id: ModeId,
}

impl ResultResolver {
    /// Create a new result mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: TetrominoMode::RESULT_ID,
        }
    }
}

impl Default for ResultResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for ResultResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
