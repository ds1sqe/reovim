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
mod tests {
    use {
        reovim_driver_input::{KeyCode, KeymapQuery},
        reovim_kernel::api::v1::CommandId,
    };

    use {super::*, crate::state::ClientId};

    // =========================================================================
    // Mock keymaps
    // =========================================================================

    struct NotFoundKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for NotFoundKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::NotFound
        }
    }

    struct ExactOnlyKeymap(CommandId);

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for ExactOnlyKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::ExactOnly(self.0.clone())
        }
    }

    struct ExactWithLongerKeymap(CommandId);

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for ExactWithLongerKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::ExactWithLonger {
                exact: self.0.clone(),
            }
        }
    }

    struct PrefixOnlyKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for PrefixOnlyKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::PrefixOnly
        }
    }

    fn play_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = TetrominoMode::PLAY_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn paused_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = TetrominoMode::PAUSED_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn menu_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = TetrominoMode::MENU_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn lobby_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = TetrominoMode::LOBBY_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn room_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = TetrominoMode::ROOM_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn char_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    // =========================================================================
    // PlayResolver
    // =========================================================================

    #[test]
    fn play_mode_id() {
        let resolver = PlayResolver::new();
        assert_eq!(resolver.mode_id(), &TetrominoMode::PLAY_ID);
    }

    #[test]
    fn play_no_inheritance() {
        let resolver = PlayResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn play_default() {
        let resolver = PlayResolver::default();
        assert_eq!(resolver.mode_id(), &TetrominoMode::PLAY_ID);
    }

    #[test]
    fn play_not_found_clears_pending() {
        let resolver = PlayResolver::new();
        let keymap = NotFoundKeymap;
        let input = play_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PLAY_ID);

        let result = resolver.resolve_with_keymap(&char_key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn play_exact_only_clears_pending() {
        let resolver = PlayResolver::new();
        let cmd = crate::ids::MOVE_LEFT;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = play_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PLAY_ID);

        let result = resolver.resolve_with_keymap(&char_key('h'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn play_exact_with_longer_clears_pending() {
        let resolver = PlayResolver::new();
        let cmd = crate::ids::MOVE_RIGHT;
        let keymap = ExactWithLongerKeymap(cmd.clone());
        let input = play_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PLAY_ID);

        let result = resolver.resolve_with_keymap(&char_key('l'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    fn play_prefix_keeps_pending() {
        let resolver = PlayResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = play_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PLAY_ID);

        let result = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert!(!resolver.pending_keys().is_empty());
    }

    #[test]
    fn play_reset_clears_pending() {
        let mut resolver = PlayResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = play_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PLAY_ID);

        let _ = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(!resolver.pending_keys().is_empty());

        resolver.reset();
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    fn play_resolve_with_extensions_checks_tick() {
        let resolver = PlayResolver::new();
        let keymap = NotFoundKeymap;
        let input = play_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::PLAY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // Initialize tetromino state
        client.get_or_insert::<TetrominoState>().start_game();

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        // Key is not found, but the tick check should not panic
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn play_resolve_with_extensions_applies_garbage() {
        let resolver = PlayResolver::new();
        let keymap = NotFoundKeymap;
        let input = play_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::PLAY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // Set up multiplayer state
        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.start_game();
        tetro.multiplayer = true;
        tetro.client_id = Some(ClientId(1));

        // Set up lobby with room
        let lobby = shared.get_or_insert::<TetrominoLobbyState>();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.broadcast_garbage(room_id, ClientId(2), 1);

        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.current_room = Some(room_id);

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        assert!(matches!(result, ResolveResult::NotHandled));
        // Garbage should have been applied (bottom row should have blocks)
        let game = client
            .get::<TetrominoState>()
            .unwrap()
            .game
            .as_ref()
            .unwrap();
        let bottom_row = &game.board[crate::game::BOARD_HEIGHT - 1];
        // At least some cells should be non-None (garbage blocks)
        assert!(bottom_row.iter().any(Option::is_some));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn play_resolve_with_extensions_transitions_to_result() {
        let resolver = PlayResolver::new();
        let keymap = NotFoundKeymap;
        let input = play_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::PLAY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.active = true;
        tetro.screen = TetrominoScreen::Result;
        tetro.start_game();

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        match result {
            ResolveResult::Execute(cmd, _) => assert_eq!(cmd, crate::ids::ENTER_RESULT),
            other => panic!("expected Execute(ENTER_RESULT), got {other:?}"),
        }
    }

    // =========================================================================
    // PausedResolver
    // =========================================================================

    #[test]
    fn paused_mode_id() {
        let resolver = PausedResolver::new();
        assert_eq!(resolver.mode_id(), &TetrominoMode::PAUSED_ID);
    }

    #[test]
    fn paused_no_inheritance() {
        let resolver = PausedResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn paused_default() {
        let resolver = PausedResolver::default();
        assert_eq!(resolver.mode_id(), &TetrominoMode::PAUSED_ID);
    }

    #[test]
    fn paused_not_found() {
        let resolver = PausedResolver::new();
        let keymap = NotFoundKeymap;
        let input = paused_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PAUSED_ID);

        let result = resolver.resolve_with_keymap(&char_key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn paused_exact_only() {
        let resolver = PausedResolver::new();
        let cmd = crate::ids::PAUSE;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = paused_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PAUSED_ID);

        let result = resolver.resolve_with_keymap(&char_key('p'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn paused_exact_with_longer() {
        let resolver = PausedResolver::new();
        let cmd = crate::ids::QUIT;
        let keymap = ExactWithLongerKeymap(cmd.clone());
        let input = paused_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PAUSED_ID);

        let result = resolver.resolve_with_keymap(&char_key('q'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
    }

    #[test]
    fn paused_prefix_only() {
        let resolver = PausedResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = paused_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::PAUSED_ID);

        let result = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }

    // =========================================================================
    // MenuResolver
    // =========================================================================

    #[test]
    fn menu_mode_id() {
        let resolver = MenuResolver::new();
        assert_eq!(resolver.mode_id(), &TetrominoMode::MENU_ID);
    }

    #[test]
    fn menu_no_inheritance() {
        let resolver = MenuResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn menu_default() {
        let resolver = MenuResolver::default();
        assert_eq!(resolver.mode_id(), &TetrominoMode::MENU_ID);
    }

    #[test]
    fn menu_not_found() {
        let resolver = MenuResolver::new();
        let keymap = NotFoundKeymap;
        let input = menu_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::MENU_ID);

        let result = resolver.resolve_with_keymap(&char_key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn menu_exact_only() {
        let resolver = MenuResolver::new();
        let cmd = crate::ids::START_SINGLE;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = menu_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::MENU_ID);

        let result = resolver.resolve_with_keymap(&char_key('s'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
    }

    #[test]
    fn menu_prefix_only() {
        let resolver = MenuResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = menu_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::MENU_ID);

        let result = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }

    // =========================================================================
    // LobbyResolver
    // =========================================================================

    #[test]
    fn lobby_mode_id() {
        let resolver = LobbyResolver::new();
        assert_eq!(resolver.mode_id(), &TetrominoMode::LOBBY_ID);
    }

    #[test]
    fn lobby_no_inheritance() {
        let resolver = LobbyResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn lobby_default() {
        let resolver = LobbyResolver::default();
        assert_eq!(resolver.mode_id(), &TetrominoMode::LOBBY_ID);
    }

    #[test]
    fn lobby_not_found() {
        let resolver = LobbyResolver::new();
        let keymap = NotFoundKeymap;
        let input = lobby_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::LOBBY_ID);

        let result = resolver.resolve_with_keymap(&char_key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn lobby_digit_joins_room() {
        let resolver = LobbyResolver::new();
        let keymap = NotFoundKeymap;
        let input = lobby_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::LOBBY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // Set up lobby with a room
        let lobby = shared.get_or_insert::<TetrominoLobbyState>();
        let room_id = lobby.create_room();

        // Set up client with client_id
        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.client_id = Some(ClientId(1));

        let result = resolver.resolve_with_extensions(
            &char_key('1'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        // Should delegate to JoinRoomByIndex command handler
        match result {
            ResolveResult::Execute(cmd, _) => assert_eq!(cmd, crate::ids::JOIN_ROOM_BY_INDEX),
            other => panic!("expected Execute(JOIN_ROOM_BY_INDEX), got {other:?}"),
        }
        let tetro = client.get::<TetrominoState>().unwrap();
        assert_eq!(tetro.current_room, Some(room_id));
        assert_eq!(tetro.screen, TetrominoScreen::Room);
    }

    #[test]
    fn lobby_digit_no_room() {
        let resolver = LobbyResolver::new();
        let keymap = NotFoundKeymap;
        let input = lobby_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::LOBBY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // No rooms in lobby
        shared.get_or_insert::<TetrominoLobbyState>();
        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.client_id = Some(ClientId(1));

        let result = resolver.resolve_with_extensions(
            &char_key('1'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn lobby_digit_no_client_id() {
        let resolver = LobbyResolver::new();
        let keymap = NotFoundKeymap;
        let input = lobby_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::LOBBY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        let lobby = shared.get_or_insert::<TetrominoLobbyState>();
        lobby.create_room();
        // No client_id set
        client.get_or_insert::<TetrominoState>();

        let result = resolver.resolve_with_extensions(
            &char_key('1'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn lobby_digit_no_lobby_state() {
        let resolver = LobbyResolver::new();
        let keymap = NotFoundKeymap;
        let input = lobby_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::LOBBY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // No lobby state in shared
        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.client_id = Some(ClientId(1));

        let result = resolver.resolve_with_extensions(
            &char_key('1'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn lobby_non_digit_delegates_keymap() {
        let resolver = LobbyResolver::new();
        let keymap = ExactOnlyKeymap(crate::ids::CREATE_ROOM);
        let input = lobby_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::LOBBY_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        let result = resolver.resolve_with_extensions(
            &char_key('c'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        assert!(matches!(result, ResolveResult::Execute(_, _)));
    }

    // =========================================================================
    // RoomResolver
    // =========================================================================

    #[test]
    fn room_mode_id() {
        let resolver = RoomResolver::new();
        assert_eq!(resolver.mode_id(), &TetrominoMode::ROOM_ID);
    }

    #[test]
    fn room_no_inheritance() {
        let resolver = RoomResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn room_default() {
        let resolver = RoomResolver::default();
        assert_eq!(resolver.mode_id(), &TetrominoMode::ROOM_ID);
    }

    #[test]
    fn room_not_found() {
        let resolver = RoomResolver::new();
        let keymap = NotFoundKeymap;
        let input = room_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::ROOM_ID);

        let result = resolver.resolve_with_keymap(&char_key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn room_detects_match_start() {
        let resolver = RoomResolver::new();
        let keymap = NotFoundKeymap;
        let input = room_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::ROOM_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // Set up lobby with in-progress room
        let lobby = shared.get_or_insert::<TetrominoLobbyState>();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.start_match(room_id);

        // Set up client in room
        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.current_room = Some(room_id);
        tetro.screen = TetrominoScreen::Room;
        tetro.client_id = Some(ClientId(2));

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        match result {
            ResolveResult::Execute(cmd, _) => assert_eq!(cmd, crate::ids::START_MATCH),
            other => panic!("expected Execute(START_MATCH), got {other:?}"),
        }
        let tetro = client.get::<TetrominoState>().unwrap();
        assert_eq!(tetro.screen, TetrominoScreen::Game);
        assert!(tetro.multiplayer);
        assert!(tetro.game.is_some());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn room_detects_match_start_when_tick_already_started_game() {
        let resolver = RoomResolver::new();
        let keymap = NotFoundKeymap;
        let input = room_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::ROOM_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // Set up lobby with in-progress room
        let lobby = shared.get_or_insert::<TetrominoLobbyState>();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.start_match(room_id);

        // Tick catch-up already set screen=Game and started game
        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.current_room = Some(room_id);
        tetro.screen = TetrominoScreen::Game;
        tetro.multiplayer = true;
        tetro.start_game();

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        // Should still trigger mode transition even though game already started
        match result {
            ResolveResult::Execute(cmd, _) => assert_eq!(cmd, crate::ids::START_MATCH),
            other => panic!("expected Execute(START_MATCH), got {other:?}"),
        }
    }

    #[test]
    fn room_detects_countdown() {
        let resolver = RoomResolver::new();
        let keymap = NotFoundKeymap;
        let input = room_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::ROOM_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // Set up lobby with countdown room
        let lobby = shared.get_or_insert::<TetrominoLobbyState>();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));
        lobby.join_room(room_id, ClientId(2));
        lobby.start_countdown(room_id);

        // Client is still on Room screen (hasn't seen countdown yet)
        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.current_room = Some(room_id);
        tetro.screen = TetrominoScreen::Room;

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        // Should delegate to keymap (no mode transition), but screen updated
        assert!(matches!(result, ResolveResult::NotHandled));
        let tetro = client.get::<TetrominoState>().unwrap();
        assert_eq!(tetro.screen, TetrominoScreen::Countdown);
    }

    #[test]
    fn room_no_match_start_delegates() {
        let resolver = RoomResolver::new();
        let keymap = NotFoundKeymap;
        let input = room_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::ROOM_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        // Set up lobby with waiting room
        let lobby = shared.get_or_insert::<TetrominoLobbyState>();
        let room_id = lobby.create_room();
        lobby.join_room(room_id, ClientId(1));

        let tetro = client.get_or_insert::<TetrominoState>();
        tetro.current_room = Some(room_id);
        tetro.screen = TetrominoScreen::Room;

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn room_no_room_id_delegates() {
        let resolver = RoomResolver::new();
        let keymap = NotFoundKeymap;
        let input = room_input(&keymap);
        let mut mode_state = ModeState::new(TetrominoMode::ROOM_ID);
        let mut shared = ExtensionMap::new();
        let mut client = ExtensionMap::new();

        client.get_or_insert::<TetrominoState>();

        let result = resolver.resolve_with_extensions(
            &char_key('x'),
            &mut mode_state,
            &input,
            &mut shared,
            &mut client,
        );
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn room_exact_only() {
        let resolver = RoomResolver::new();
        let cmd = crate::ids::READY_TOGGLE;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = room_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::ROOM_ID);

        let result = resolver.resolve_with_keymap(&char_key('r'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
    }

    #[test]
    fn room_prefix_only() {
        let resolver = RoomResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = room_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::ROOM_ID);

        let result = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }

    // =========================================================================
    // ResultResolver
    // =========================================================================

    fn result_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = TetrominoMode::RESULT_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    #[test]
    fn result_mode_id() {
        let resolver = ResultResolver::new();
        assert_eq!(resolver.mode_id(), &TetrominoMode::RESULT_ID);
    }

    #[test]
    fn result_no_inheritance() {
        let resolver = ResultResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn result_default() {
        let resolver = ResultResolver::default();
        assert_eq!(resolver.mode_id(), &TetrominoMode::RESULT_ID);
    }

    #[test]
    fn result_not_found() {
        let resolver = ResultResolver::new();
        let keymap = NotFoundKeymap;
        let input = result_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::RESULT_ID);

        let result = resolver.resolve_with_keymap(&char_key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn result_exact_only() {
        let resolver = ResultResolver::new();
        let cmd = crate::ids::RETURN_LOBBY;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = result_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::RESULT_ID);

        let result = resolver.resolve_with_keymap(&char_key('q'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
    }

    #[test]
    fn result_prefix_only() {
        let resolver = ResultResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = result_input(&keymap);
        let mut state = ModeState::new(TetrominoMode::RESULT_ID);

        let result = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }
}
