//! Tetromino mode key resolvers.
//!
//! Two resolvers for the two tetromino modes:
//! - `PlayResolver` for `tetromino:PLAY` (game keys via keymap, tick check)
//! - `PausedResolver` for `tetromino:PAUSED` (only unpause/quit via keymap)

use std::sync::RwLock;

use {
    reovim_driver_input::{
        KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, ResolveContext,
        ResolveInput, ResolveResult,
    },
    reovim_driver_session::ExtensionMap,
    reovim_kernel::api::v1::ModeId,
};

use crate::{modes::TetrominoMode, state::TetrominoState};

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
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
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

#[cfg(test)]
mod tests {
    use {
        reovim_driver_input::{KeyCode, KeymapQuery},
        reovim_kernel::api::v1::CommandId,
    };

    use super::*;

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
}
