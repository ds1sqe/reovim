//! Which-key plugin state

use std::sync::Arc;

use {
    arc_swap::ArcSwap,
    reovim_core::{
        bind::KeymapScope,
        keystroke::{KeySequence, Keystroke},
    },
};

use crate::saturator::WhichKeySaturatorHandle;

/// Holder for the cache during `init_state` -> `boot` transition
pub struct WhichKeyCacheHolder {
    pub cache: Arc<ArcSwap<WhichKeyCache>>,
}

/// Full which-key state with saturator handle
pub struct WhichKeyState {
    /// Lock-free cache for render thread
    pub cache: Arc<ArcSwap<WhichKeyCache>>,
    /// Handle to send requests to background task
    pub saturator: WhichKeySaturatorHandle,
}

/// Cached filter results (lock-free reads via `ArcSwap`)
#[derive(Debug, Clone)]
pub struct WhichKeyCache {
    /// Initial prefix from `?` trigger (e.g., `g` from `g?`)
    pub initial_prefix: KeySequence,
    /// Current filter prefix (initial + typed keys after opening)
    pub current_filter: KeySequence,
    /// Current mode scope for filtering
    pub scope: KeymapScope,
    /// Filtered bindings matching the current filter
    pub bindings: Vec<BindingEntry>,
    /// Whether the panel is currently visible
    pub visible: bool,
}

impl Default for WhichKeyCache {
    fn default() -> Self {
        Self {
            initial_prefix: KeySequence::default(),
            current_filter: KeySequence::default(),
            scope: KeymapScope::DefaultNormal,
            bindings: Vec::new(),
            visible: false,
        }
    }
}

impl WhichKeyCache {
    /// Append a key to the current filter
    pub fn push_key(&mut self, keystroke: Keystroke) {
        self.current_filter.push(keystroke);
    }

    /// Remove the last key from the current filter (if beyond initial prefix)
    pub fn pop_key(&mut self) -> bool {
        if self.current_filter.len() > self.initial_prefix.len() {
            self.current_filter.pop();
            true
        } else {
            false
        }
    }

    /// Reset the filter and close the panel
    pub fn close(&mut self) {
        self.visible = false;
        self.initial_prefix = KeySequence::default();
        self.current_filter = KeySequence::default();
        self.bindings.clear();
    }

    /// Open with a prefix
    pub fn open(&mut self, prefix: KeySequence, scope: KeymapScope) {
        self.visible = true;
        self.initial_prefix = prefix.clone();
        self.current_filter = prefix;
        self.scope = scope;
    }
}

/// A single binding entry to display
#[derive(Debug, Clone)]
pub struct BindingEntry {
    /// The key(s) to press after the pending prefix
    pub suffix: KeySequence,
    /// Display text (from hint or command description)
    pub description: String,
    /// Group name for categorization
    #[allow(dead_code)] // Will be used for grouping display
    pub group: Option<&'static str>,
}
