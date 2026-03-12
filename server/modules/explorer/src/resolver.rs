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
#[path = "resolver_tests.rs"]
mod tests;
