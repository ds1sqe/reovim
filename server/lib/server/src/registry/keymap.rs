//! Keymap registry for mapping key sequences to commands.
//!
//! Supports multi-key sequences like `gg`, `<C-w>h`, etc. The registry
//! can distinguish between:
//! - **Full match**: The key sequence maps to a command
//! - **Prefix match**: The sequence is a prefix of one or more bindings
//! - **No match**: The sequence doesn't match anything
//!
//! # Layered Bindings (Epic #353)
//!
//! Bindings are organized into layers with priority ordering:
//! - **User** (highest): User configuration overrides
//! - **Policy**: Policy module defaults (Vim, Emacs, etc.)
//! - **Base** (lowest): Mechanism defaults (rarely used)
//!
//! Higher layers override lower layers for the same key sequence.

use std::{collections::HashMap, sync::Arc};

use {
    reovim_kernel::{
        api::v1::{CommandId, ModeId, ModuleId},
        profile_scope,
    },
    reovim_subsys_input_contracts::{
        BindingInfo, BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult,
        KeyLookupState, KeySequence, KeymapQuery,
    },
};

/// Entry in the keymap registry with layer and ownership tracking.
#[derive(Clone)]
struct KeybindingEntry {
    /// The command ID to execute.
    command: CommandId,
    /// The layer this binding belongs to.
    layer: BindingLayer,
    /// The module that owns this keybinding (if any).
    owner: Option<ModuleId>,
    /// Whether this entry marks the binding as removed.
    removed: bool,
    /// Human-readable description for which-key / help display.
    description: &'static str,
    /// Category for which-key grouping/filtering (e.g., "motion", "operator").
    category: Option<&'static str>,
}

/// Registry for keybindings with layered composition.
///
/// Maps (mode, key sequence) pairs to command IDs. Supports multi-key
/// sequences with prefix detection for proper handling of sequences
/// like `gg` or `<C-w>h`.
///
/// The registry uses a configurable default lookup policy. By default it
/// uses [`EagerLookupPolicy`] (mechanism default). Bootstrap wires
/// the Vim-specific policy from the vim module.
#[derive(Clone)]
pub struct KeymapRegistry {
    /// Bindings organized by mode, then by key sequence, then by layer.
    entries: HashMap<ModeId, HashMap<KeySequence, Vec<KeybindingEntry>>>,
    /// Default policy for `lookup()`. Configurable via `set_default_policy()`.
    default_policy: Arc<dyn KeyLookupPolicy>,
}

impl Default for KeymapRegistry {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            default_policy: Arc::new(EagerLookupPolicy),
        }
    }
}

impl KeymapRegistry {
    /// Create a new empty keymap registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ========================================================================
    // Layered Registration (Epic #353)
    // ========================================================================

    /// Register a keybinding at a specific layer.
    ///
    /// Higher layers override lower layers for the same key sequence.
    pub fn register_at_layer(
        &mut self,
        layer: BindingLayer,
        mode: &ModeId,
        keys: KeySequence,
        command: CommandId,
        description: &'static str,
        category: Option<&'static str>,
    ) {
        let mode_entries = self.entries.entry(mode.clone()).or_default();
        let key_entries = mode_entries.entry(keys).or_default();

        // Remove existing entry at same layer (if any)
        key_entries.retain(|e| e.layer != layer);

        // Add new entry
        key_entries.push(KeybindingEntry {
            command,
            layer,
            owner: None,
            removed: false,
            description,
            category,
        });

        // Sort by layer (highest first) for efficient lookup
        key_entries.sort_by_key(|entry| std::cmp::Reverse(entry.layer));
    }

    /// Register a keybinding at a specific layer with module ownership.
    pub fn register_at_layer_for_module(
        &mut self,
        mode: &ModeId,
        keys: KeySequence,
        info: BindingInfo,
        owner: ModuleId,
    ) {
        let mode_entries = self.entries.entry(mode.clone()).or_default();
        let key_entries = mode_entries.entry(keys).or_default();

        key_entries.retain(|e| e.layer != info.layer);

        key_entries.push(KeybindingEntry {
            command: info.command,
            layer: info.layer,
            owner: Some(owner),
            removed: false,
            description: info.description,
            category: info.category,
        });

        key_entries.sort_by_key(|entry| std::cmp::Reverse(entry.layer));
    }

    /// Get the effective binding for a key sequence (highest layer wins).
    #[must_use]
    pub fn get_binding(&self, mode: &ModeId, keys: &KeySequence) -> Option<CommandId> {
        self.entries
            .get(mode)
            .and_then(|m| m.get(keys))
            .and_then(|entries| entries.first())
            .filter(|e| !e.removed)
            .map(|e| e.command.clone())
    }

    /// Pure query - reports facts about what exists.
    #[must_use]
    pub fn query(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupState {
        profile_scope!("keymap_query", "server::keymap");

        let exact = self.get_binding(mode, keys);
        let has_longer = self.has_longer_bindings(mode, keys);

        match (exact, has_longer) {
            (Some(cmd), true) => KeyLookupState::ExactWithLonger { exact: cmd },
            (Some(cmd), false) => KeyLookupState::ExactOnly(cmd),
            (None, true) => KeyLookupState::PrefixOnly,
            (None, false) => KeyLookupState::NotFound,
        }
    }

    /// Check if longer bindings exist for a key sequence.
    #[must_use]
    pub fn has_longer_bindings(&self, mode: &ModeId, keys: &KeySequence) -> bool {
        self.entries.get(mode).is_some_and(|mode_entries| {
            mode_entries
                .iter()
                .any(|(k, entries)| k.starts_with(keys) && k != keys && !entries.is_empty())
        })
    }

    /// Clear all bindings at a specific layer for a mode.
    pub fn clear_layer(&mut self, layer: BindingLayer, mode: &ModeId) {
        if let Some(mode_entries) = self.entries.get_mut(mode) {
            for entries in mode_entries.values_mut() {
                entries.retain(|e| e.layer != layer);
            }
            mode_entries.retain(|_, entries| !entries.is_empty());
        }
        self.entries
            .retain(|_, mode_entries| !mode_entries.is_empty());
    }

    /// Mark a binding as removed at a specific layer.
    pub fn remove_at_layer(&mut self, layer: BindingLayer, mode: &ModeId, keys: KeySequence) {
        let mode_entries = self.entries.entry(mode.clone()).or_default();
        let key_entries = mode_entries.entry(keys).or_default();

        key_entries.retain(|e| e.layer != layer);

        key_entries.push(KeybindingEntry {
            command: CommandId::new(ModuleId::new("system"), "noop"),
            layer,
            owner: None,
            removed: true,
            description: "",
            category: None,
        });

        key_entries.sort_by_key(|entry| std::cmp::Reverse(entry.layer));
    }

    /// Remove all keybindings owned by a module.
    pub fn unregister_for_module(&mut self, module: &ModuleId) -> usize {
        let mut removed = 0;
        for mode_entries in self.entries.values_mut() {
            for entries in mode_entries.values_mut() {
                let before = entries.len();
                entries.retain(|e| e.owner.as_ref() != Some(module));
                removed += before - entries.len();
            }
            mode_entries.retain(|_, entries| !entries.is_empty());
        }
        self.entries
            .retain(|_, mode_entries| !mode_entries.is_empty());
        removed
    }

    /// Register a keybinding from a string at the Policy layer.
    ///
    /// Uses empty description and no category. For rich metadata, use
    /// [`register_at_layer()`] directly.
    pub fn register_str(&mut self, mode: &ModeId, keys: &str, command: CommandId) -> bool {
        KeySequence::parse(keys).is_some_and(|seq| {
            self.register_at_layer(BindingLayer::Policy, mode, seq, command, "", None);
            true
        })
    }

    /// Set the default lookup policy for `lookup()`.
    ///
    /// By default, the registry uses [`EagerLookupPolicy`]. Call this to
    /// install a different policy (e.g., `VimLookupPolicy` from the vim module).
    pub fn set_default_policy(&mut self, policy: Arc<dyn KeyLookupPolicy>) {
        self.default_policy = policy;
    }

    /// Look up a key sequence in a mode using the default policy.
    #[must_use]
    pub fn lookup(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        profile_scope!("keymap_lookup", "server::keymap");
        self.lookup_with_policy(mode, keys, &*self.default_policy)
    }

    /// Look up a key sequence in a mode with a specific policy.
    #[must_use]
    pub fn lookup_with_policy(
        &self,
        mode: &ModeId,
        keys: &KeySequence,
        policy: &dyn KeyLookupPolicy,
    ) -> KeyLookupResult {
        profile_scope!("keymap_lookup_with_policy", "server::keymap");
        policy.resolve(self.query(mode, keys))
    }

    // ========================================================================
    // Query methods
    // ========================================================================

    /// Get all bindings for a mode (effective bindings only).
    #[must_use]
    pub fn bindings_for_mode(&self, mode: &ModeId) -> Vec<(&KeySequence, &CommandId)> {
        self.entries
            .get(mode)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, entries)| entries.first().map(|e| (k, &e.command)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all bindings that start with a given prefix.
    ///
    /// Returns `BindingInfo` with full metadata (command, description,
    /// category, layer) for each matching binding.
    #[must_use]
    pub fn bindings_with_prefix(
        &self,
        mode: &ModeId,
        prefix: &KeySequence,
    ) -> Vec<(KeySequence, BindingInfo)> {
        self.entries
            .get(mode)
            .map(|mode_entries| {
                mode_entries
                    .iter()
                    .filter(|(keys, _)| keys.starts_with(prefix) && *keys != prefix)
                    .filter_map(|(keys, entries)| {
                        entries.first().and_then(|entry| {
                            if entry.removed {
                                None
                            } else {
                                Some((
                                    keys.clone(),
                                    BindingInfo::new(
                                        entry.command.clone(),
                                        entry.description,
                                        entry.category,
                                        entry.layer,
                                    ),
                                ))
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the number of unique key sequences with bindings for a mode.
    #[must_use]
    pub fn binding_count(&self, mode: &ModeId) -> usize {
        self.entries
            .get(mode)
            .map_or(0, |m| m.values().filter(|entries| !entries.is_empty()).count())
    }

    /// Get total number of unique key sequences with bindings across all modes.
    #[must_use]
    pub fn total_bindings(&self) -> usize {
        self.entries
            .values()
            .map(|m| m.values().filter(|entries| !entries.is_empty()).count())
            .sum()
    }

    /// Get all modes that have bindings.
    pub fn modes(&self) -> impl Iterator<Item = &ModeId> {
        self.entries
            .iter()
            .filter(|(_, m)| m.values().any(|entries| !entries.is_empty()))
            .map(|(mode, _)| mode)
    }

    /// Check if the registry has any bindings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() || self.entries.values().all(|m| m.values().all(Vec::is_empty))
    }
}

impl std::fmt::Debug for KeymapRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeymapRegistry")
            .field("modes", &self.entries.keys().collect::<Vec<_>>())
            .field("total_bindings", &self.total_bindings())
            .field("default_policy", &"<dyn KeyLookupPolicy>")
            .finish()
    }
}

// ============================================================================
// KeymapQuery Implementation (Epic #353)
// ============================================================================

impl KeymapQuery for KeymapRegistry {
    fn query(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupState {
        Self::query(self, mode, keys)
    }

    fn has_longer_bindings(&self, mode: &ModeId, keys: &KeySequence) -> bool {
        Self::has_longer_bindings(self, mode, keys)
    }

    fn get_exact(&self, mode: &ModeId, keys: &KeySequence) -> Option<CommandId> {
        self.get_binding(mode, keys)
    }

    fn bindings_with_prefix(
        &self,
        mode: &ModeId,
        prefix: &KeySequence,
    ) -> Vec<(KeySequence, BindingInfo)> {
        Self::bindings_with_prefix(self, mode, prefix)
    }
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
