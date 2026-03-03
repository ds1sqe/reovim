//! Explorer mode key resolvers (#523).
//!
//! Two resolvers for the explorer's two modes:
//! - `BrowseResolver` for `explorer:EXPLORER` (navigation keybindings)
//! - `InputResolver` for `explorer:EXPLORER_INPUT` (CR/Esc/BS bound, chars → `TextInputSink`)

use std::sync::RwLock;

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveContext, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::{modes::ExplorerMode, state::ExplorerState};

/// Key resolver for explorer browse mode.
///
/// Accumulates multi-key sequences (`gg`, `<Space>e`) and resolves
/// via the keymap. Unmatched keys are ignored (no inheritance).
pub struct BrowseResolver {
    mode_id: ModeId,
    pending_keys: RwLock<KeySequence>,
}

impl BrowseResolver {
    /// Create a new browse mode resolver.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new is not const-stable
    pub fn new() -> Self {
        Self {
            mode_id: ExplorerMode::BROWSE_ID,
            pending_keys: RwLock::new(KeySequence::new()),
        }
    }
}

impl Default for BrowseResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for BrowseResolver {
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
        // Accumulate key into pending sequence
        self.pending_keys.write().expect("lock poisoned").push(*key);
        let keys = self.pending_keys.read().expect("lock poisoned").clone();

        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                // Clear pending keys on match
                self.pending_keys.write().expect("lock poisoned").clear();
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => {
                // Clear pending keys on mismatch
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

/// Key resolver for explorer input mode.
///
/// Printable characters are routed to `ExplorerState`'s `TextInputSink`
/// via `ResolveResult::insert_char_to`. Only special keys (CR, Esc, BS)
/// are resolved via the keymap.
pub struct InputResolver {
    mode_id: ModeId,
}

impl InputResolver {
    /// Create a new input mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: ExplorerMode::INPUT_ID,
        }
    }

    /// Check if a key event is an insertable character (no Ctrl/Alt modifiers).
    const fn is_insertable(key: &KeyEvent) -> Option<char> {
        if key.modifiers.contains(Modifiers::CTRL) || key.modifiers.contains(Modifiers::ALT) {
            return None;
        }
        match key.code {
            KeyCode::Char(c) => Some(c),
            _ => None,
        }
    }
}

impl Default for InputResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for InputResolver {
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
        // Route printable chars to ExplorerState's TextInputSink
        if let Some(c) = Self::is_insertable(key) {
            return ResolveResult::insert_char_to::<ExplorerState>(c);
        }

        // Special keys (CR, Esc, BS) resolved via keymap
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

    fn browse_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = ExplorerMode::BROWSE_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn input_input(keymap: &dyn KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = ExplorerMode::INPUT_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    fn char_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn enter_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter)
    }

    // =========================================================================
    // BrowseResolver
    // =========================================================================

    #[test]
    fn browse_mode_id() {
        let resolver = BrowseResolver::new();
        assert_eq!(resolver.mode_id(), &ExplorerMode::BROWSE_ID);
    }

    #[test]
    fn browse_no_inheritance() {
        let resolver = BrowseResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn browse_default() {
        let resolver = BrowseResolver::default();
        assert_eq!(resolver.mode_id(), &ExplorerMode::BROWSE_ID);
    }

    #[test]
    fn browse_not_found_clears_pending() {
        let resolver = BrowseResolver::new();
        let keymap = NotFoundKeymap;
        let input = browse_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::BROWSE_ID);

        let result = resolver.resolve_with_keymap(&char_key('x'), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    fn browse_exact_only_clears_pending() {
        let resolver = BrowseResolver::new();
        let cmd = crate::ids::CURSOR_DOWN;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = browse_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::BROWSE_ID);

        let result = resolver.resolve_with_keymap(&char_key('j'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    fn browse_exact_with_longer_clears_pending() {
        let resolver = BrowseResolver::new();
        let cmd = crate::ids::GOTO_FIRST;
        let keymap = ExactWithLongerKeymap(cmd.clone());
        let input = browse_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::BROWSE_ID);

        let result = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
        assert!(resolver.pending_keys().is_empty());
    }

    #[test]
    fn browse_prefix_keeps_pending() {
        let resolver = BrowseResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = browse_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::BROWSE_ID);

        let result = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
        assert!(!resolver.pending_keys().is_empty());
    }

    #[test]
    fn browse_reset_clears_pending() {
        let mut resolver = BrowseResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = browse_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::BROWSE_ID);

        let _ = resolver.resolve_with_keymap(&char_key('g'), &mut state, &input);
        assert!(!resolver.pending_keys().is_empty());

        resolver.reset();
        assert!(resolver.pending_keys().is_empty());
    }

    // =========================================================================
    // InputResolver
    // =========================================================================

    #[test]
    fn input_mode_id() {
        let resolver = InputResolver::new();
        assert_eq!(resolver.mode_id(), &ExplorerMode::INPUT_ID);
    }

    #[test]
    fn input_no_inheritance() {
        let resolver = InputResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn input_default() {
        let resolver = InputResolver::default();
        assert_eq!(resolver.mode_id(), &ExplorerMode::INPUT_ID);
    }

    #[test]
    fn input_char_routes_to_text_input_sink() {
        let resolver = InputResolver::new();
        let keymap = NotFoundKeymap;
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        // Printable chars are routed to ExplorerState's TextInputSink,
        // NOT to keymap lookup.
        let result = resolver.resolve_with_keymap(&char_key('a'), &mut state, &input);
        assert!(
            matches!(result, ResolveResult::InsertChar { char: 'a', .. }),
            "expected InsertChar, got {result:?}"
        );
    }

    #[test]
    fn input_exact_enter() {
        let resolver = InputResolver::new();
        let cmd = crate::ids::CONFIRM_INPUT;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        let result = resolver.resolve_with_keymap(&enter_key(), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
    }

    #[test]
    fn input_exact_with_longer() {
        let resolver = InputResolver::new();
        let cmd = crate::ids::CANCEL_INPUT;
        let keymap = ExactWithLongerKeymap(cmd.clone());
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        let result = resolver.resolve_with_keymap(&enter_key(), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute, got {other:?}"),
        }
    }

    #[test]
    fn input_prefix_only() {
        let resolver = InputResolver::new();
        let keymap = PrefixOnlyKeymap;
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        let result = resolver.resolve_with_keymap(&enter_key(), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }

    #[test]
    fn input_routes_various_chars() {
        let resolver = InputResolver::new();
        let keymap = NotFoundKeymap;
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        for c in ['m', 'a', 'i', 'n', '.', 'r', 's', ' ', 'Z', '0', '_', '-'] {
            let result = resolver.resolve_with_keymap(&char_key(c), &mut state, &input);
            assert!(
                matches!(result, ResolveResult::InsertChar { char: ch, .. } if ch == c),
                "expected InsertChar('{c}'), got {result:?}"
            );
        }
    }

    #[test]
    fn input_ctrl_char_not_inserted() {
        let resolver = InputResolver::new();
        let keymap = NotFoundKeymap;
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        let ctrl_a = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL);
        let result = resolver.resolve_with_keymap(&ctrl_a, &mut state, &input);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "Ctrl+a should not insert, got {result:?}"
        );
    }

    #[test]
    fn input_alt_char_not_inserted() {
        let resolver = InputResolver::new();
        let keymap = NotFoundKeymap;
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        let alt_a = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::ALT);
        let result = resolver.resolve_with_keymap(&alt_a, &mut state, &input);
        assert!(
            matches!(result, ResolveResult::NotHandled),
            "Alt+a should not insert, got {result:?}"
        );
    }

    #[test]
    fn input_special_keys_use_keymap_not_insert() {
        let resolver = InputResolver::new();
        let cmd = crate::ids::CONFIRM_INPUT;
        let keymap = ExactOnlyKeymap(cmd.clone());
        let input = input_input(&keymap);
        let mut state = ModeState::new(ExplorerMode::INPUT_ID);

        // Enter should go through keymap, not InsertChar
        let result = resolver.resolve_with_keymap(&enter_key(), &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute for Enter, got {other:?}"),
        }

        // Backspace should go through keymap, not InsertChar
        let bs = KeyEvent::new(KeyCode::Backspace);
        let result = resolver.resolve_with_keymap(&bs, &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute for Backspace, got {other:?}"),
        }

        // Escape should go through keymap, not InsertChar
        let esc = KeyEvent::new(KeyCode::Escape);
        let result = resolver.resolve_with_keymap(&esc, &mut state, &input);
        match result {
            ResolveResult::Execute(id, _) => assert_eq!(id, cmd),
            other => panic!("expected Execute for Escape, got {other:?}"),
        }
    }
}
