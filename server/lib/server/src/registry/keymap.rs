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

use std::collections::HashMap;

use {
    reovim_driver_input::{
        BindingLayer, KeyLookupPolicy, KeyLookupResult, KeyLookupState, KeySequence, KeymapQuery,
        VimLookupPolicy,
    },
    reovim_kernel::{
        api::v1::{CommandId, ModeId, ModuleId},
        profile_scope,
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
}

/// Registry for keybindings with layered composition.
///
/// Maps (mode, key sequence) pairs to command IDs. Supports multi-key
/// sequences with prefix detection for proper handling of sequences
/// like `gg` or `<C-w>h`.
#[derive(Default, Clone)]
pub struct KeymapRegistry {
    /// Bindings organized by mode, then by key sequence, then by layer.
    entries: HashMap<ModeId, HashMap<KeySequence, Vec<KeybindingEntry>>>,
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
        });

        // Sort by layer (highest first) for efficient lookup
        key_entries.sort_by_key(|entry| std::cmp::Reverse(entry.layer));
    }

    /// Register a keybinding at a specific layer with module ownership.
    pub fn register_at_layer_for_module(
        &mut self,
        layer: BindingLayer,
        mode: &ModeId,
        keys: KeySequence,
        command: CommandId,
        owner: ModuleId,
    ) {
        let mode_entries = self.entries.entry(mode.clone()).or_default();
        let key_entries = mode_entries.entry(keys).or_default();

        key_entries.retain(|e| e.layer != layer);

        key_entries.push(KeybindingEntry {
            command,
            layer,
            owner: Some(owner),
            removed: false,
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
    pub fn register_str(&mut self, mode: &ModeId, keys: &str, command: CommandId) -> bool {
        KeySequence::parse(keys).is_some_and(|seq| {
            self.register_at_layer(BindingLayer::Policy, mode, seq, command);
            true
        })
    }

    /// Look up a key sequence in a mode (Vim-style behavior).
    #[must_use]
    pub fn lookup(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        profile_scope!("keymap_lookup", "server::keymap");
        self.lookup_with_policy(mode, keys, &VimLookupPolicy)
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
    #[must_use]
    pub fn bindings_with_prefix(
        &self,
        mode: &ModeId,
        prefix: &KeySequence,
    ) -> Vec<(KeySequence, CommandId)> {
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
                                Some((keys.clone(), entry.command.clone()))
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
    ) -> Vec<(KeySequence, CommandId)> {
        Self::bindings_with_prefix(self, mode, prefix)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    fn test_mode() -> ModeId {
        ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0)
    }

    fn test_command(name: &'static str) -> CommandId {
        CommandId::new(TEST_MODULE, name)
    }

    #[test]
    fn test_keymap_registry_new() {
        let registry = KeymapRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.total_bindings(), 0);
    }

    #[test]
    fn test_keymap_registry_register() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("j").unwrap();
        let cmd = test_command("cursor-down");

        registry.register_at_layer(BindingLayer::Policy, &mode, keys, cmd);

        assert_eq!(registry.binding_count(&mode), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_keymap_registry_lookup_found() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let cmd = test_command("cursor-down");

        registry.register_str(&mode, "j", cmd.clone());

        let keys = KeySequence::parse("j").unwrap();
        let result = registry.lookup(&mode, &keys);

        assert!(result.is_found());
        assert_eq!(result.command_id(), Some(&cmd));
    }

    #[test]
    fn test_keymap_registry_lookup_prefix() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Register `gg`
        registry.register_str(&mode, "gg", test_command("goto-top"));

        // Look up single `g` - should be prefix
        let g = KeySequence::parse("g").unwrap();
        let result = registry.lookup(&mode, &g);

        assert!(result.is_prefix());
    }

    #[test]
    fn test_keymap_registry_multi_key_sequence() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Register both `g` and `gg`
        registry.register_str(&mode, "g", test_command("goto"));
        registry.register_str(&mode, "gg", test_command("goto-top"));

        // lookup() uses VIM-STYLE semantics
        let g = KeySequence::parse("g").unwrap();
        let result = registry.lookup(&mode, &g);
        assert!(result.is_prefix());

        // query() shows the full picture
        let state = registry.query(&mode, &g);
        assert_eq!(
            state,
            KeyLookupState::ExactWithLonger {
                exact: test_command("goto")
            }
        );

        // `gg` is an exact match
        let gg = KeySequence::parse("gg").unwrap();
        let result = registry.lookup(&mode, &gg);
        assert!(result.is_found());
    }

    #[test]
    fn test_layer_override() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("d").unwrap();

        // Policy layer
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("delete"),
        );

        // User layer (overrides)
        registry.register_at_layer(
            BindingLayer::User,
            &mode,
            keys.clone(),
            test_command("custom-delete"),
        );

        // User layer wins
        let binding = registry.get_binding(&mode, &keys);
        assert_eq!(binding, Some(test_command("custom-delete")));
    }

    #[test]
    fn test_unregister_for_module() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let owner = ModuleId::new("my-module");

        let j = KeySequence::parse("j").unwrap();
        let k = KeySequence::parse("k").unwrap();
        registry.register_at_layer_for_module(
            BindingLayer::Policy,
            &mode,
            j.clone(),
            test_command("down"),
            owner.clone(),
        );
        registry.register_at_layer_for_module(
            BindingLayer::Policy,
            &mode,
            k,
            test_command("up"),
            owner.clone(),
        );

        let l = KeySequence::parse("l").unwrap();
        registry.register_at_layer(BindingLayer::Policy, &mode, l.clone(), test_command("right"));

        assert_eq!(registry.binding_count(&mode), 3);

        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 2);
        assert_eq!(registry.binding_count(&mode), 1);
        assert!(registry.lookup(&mode, &j).is_not_found());
        assert!(registry.lookup(&mode, &l).is_found());
    }

    #[test]
    fn test_clear_layer() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Add bindings at different layers
        let j = KeySequence::parse("j").unwrap();
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            j.clone(),
            test_command("policy-down"),
        );
        registry.register_at_layer(BindingLayer::User, &mode, j.clone(), test_command("user-down"));

        assert_eq!(registry.binding_count(&mode), 1);

        // Clear user layer
        registry.clear_layer(BindingLayer::User, &mode);

        // Policy binding should remain
        let binding = registry.get_binding(&mode, &j);
        assert_eq!(binding, Some(test_command("policy-down")));
    }

    #[test]
    fn test_remove_at_layer() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("x").unwrap();

        // Register at policy layer
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("delete-char"),
        );

        // Mark as removed at user layer
        registry.remove_at_layer(BindingLayer::User, &mode, keys.clone());

        // Should be filtered out
        let binding = registry.get_binding(&mode, &keys);
        assert!(binding.is_none());
    }

    #[test]
    fn test_bindings_for_mode() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "j", test_command("down"));
        registry.register_str(&mode, "k", test_command("up"));
        registry.register_str(&mode, "h", test_command("left"));

        let bindings = registry.bindings_for_mode(&mode);
        assert_eq!(bindings.len(), 3);
    }

    #[test]
    fn test_bindings_for_mode_empty() {
        let registry = KeymapRegistry::new();
        let mode = test_mode();
        let bindings = registry.bindings_for_mode(&mode);
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_bindings_with_prefix() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "g", test_command("goto"));
        registry.register_str(&mode, "gg", test_command("goto-top"));
        registry.register_str(&mode, "gj", test_command("goto-next-visual"));

        let g = KeySequence::parse("g").unwrap();
        let bindings = registry.bindings_with_prefix(&mode, &g);

        // Should not include "g" itself, only longer bindings
        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn test_bindings_with_prefix_filtered_removed() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        let gg = KeySequence::parse("gg").unwrap();
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            gg.clone(),
            test_command("goto-top"),
        );
        registry.remove_at_layer(BindingLayer::User, &mode, gg);

        let g = KeySequence::parse("g").unwrap();
        let bindings = registry.bindings_with_prefix(&mode, &g);

        // Removed binding should not appear
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_total_bindings() {
        let mut registry = KeymapRegistry::new();
        let mode1 = ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0);
        let mode2 = ModeId::with_discriminant(TEST_MODULE, "INSERT", 1);

        registry.register_str(&mode1, "j", test_command("down"));
        registry.register_str(&mode1, "k", test_command("up"));
        registry.register_str(&mode2, "a", test_command("append"));

        assert_eq!(registry.total_bindings(), 3);
    }

    #[test]
    fn test_modes_iterator() {
        let mut registry = KeymapRegistry::new();
        let mode1 = ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0);
        let mode2 = ModeId::with_discriminant(TEST_MODULE, "INSERT", 1);

        registry.register_str(&mode1, "j", test_command("down"));
        registry.register_str(&mode2, "a", test_command("append"));

        assert_eq!(registry.modes().count(), 2);
    }

    #[test]
    fn test_keymap_query_trait() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("j").unwrap();

        registry.register_str(&mode, "j", test_command("down"));

        // Test via KeymapQuery trait
        let query: &dyn KeymapQuery = &registry;
        let state = query.query(&mode, &keys);
        assert!(matches!(state, KeyLookupState::ExactOnly(_)));

        let exact = query.get_exact(&mode, &keys);
        assert_eq!(exact, Some(test_command("down")));

        let has_longer = query.has_longer_bindings(&mode, &keys);
        assert!(!has_longer);
    }

    #[test]
    fn test_keymap_query_trait_prefix() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "gg", test_command("goto-top"));

        let g = KeySequence::parse("g").unwrap();
        let query: &dyn KeymapQuery = &registry;

        let has_longer = query.has_longer_bindings(&mode, &g);
        assert!(has_longer);

        let bindings = query.bindings_with_prefix(&mode, &g);
        assert_eq!(bindings.len(), 1);
    }

    #[test]
    fn test_debug_format() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        registry.register_str(&mode, "j", test_command("down"));

        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("KeymapRegistry"));
        assert!(debug_str.contains("total_bindings"));
    }

    #[test]
    fn test_register_str_invalid() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Invalid key sequence should return false
        let result = registry.register_str(&mode, "<<invalid>>", test_command("noop"));
        assert!(!result);
    }

    #[test]
    fn test_query_not_found() {
        let registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("z").unwrap();

        let state = registry.query(&mode, &keys);
        assert_eq!(state, KeyLookupState::NotFound);
    }

    #[test]
    fn test_query_exact_only() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("j").unwrap();

        registry.register_str(&mode, "j", test_command("down"));

        let state = registry.query(&mode, &keys);
        assert_eq!(state, KeyLookupState::ExactOnly(test_command("down")));
    }

    #[test]
    fn test_query_prefix_only() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "gg", test_command("goto-top"));

        let g = KeySequence::parse("g").unwrap();
        let state = registry.query(&mode, &g);
        assert_eq!(state, KeyLookupState::PrefixOnly);
    }

    #[test]
    fn test_query_exact_with_longer() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "g", test_command("goto"));
        registry.register_str(&mode, "gg", test_command("goto-top"));

        let g = KeySequence::parse("g").unwrap();
        let state = registry.query(&mode, &g);
        assert_eq!(
            state,
            KeyLookupState::ExactWithLonger {
                exact: test_command("goto")
            }
        );
    }

    #[test]
    fn test_lookup_with_custom_policy() {
        use reovim_driver_input::KeyLookupPolicy;

        struct AlwaysExecutePolicy;
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl KeyLookupPolicy for AlwaysExecutePolicy {
            fn resolve(&self, state: KeyLookupState) -> KeyLookupResult {
                match state {
                    KeyLookupState::ExactOnly(cmd)
                    | KeyLookupState::ExactWithLonger { exact: cmd } => KeyLookupResult::Found(cmd),
                    _ => KeyLookupResult::NotFound,
                }
            }
        }

        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "g", test_command("goto"));
        registry.register_str(&mode, "gg", test_command("goto-top"));

        let g = KeySequence::parse("g").unwrap();
        let policy = AlwaysExecutePolicy;
        let result = registry.lookup_with_policy(&mode, &g, &policy);

        // Custom policy executes immediately
        assert!(result.is_found());
        assert_eq!(result.command_id(), Some(&test_command("goto")));
    }

    #[test]
    fn test_register_at_layer_for_module_replaces() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("x").unwrap();
        let owner = ModuleId::new("owner");

        // Register first
        registry.register_at_layer_for_module(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("delete1"),
            owner.clone(),
        );

        // Register again at same layer - should replace
        registry.register_at_layer_for_module(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("delete2"),
            owner,
        );

        let binding = registry.get_binding(&mode, &keys);
        assert_eq!(binding, Some(test_command("delete2")));
    }

    #[test]
    fn test_clear_layer_removes_mode_if_empty() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("j").unwrap();

        registry.register_at_layer(BindingLayer::User, &mode, keys, test_command("down"));

        assert!(!registry.is_empty());

        registry.clear_layer(BindingLayer::User, &mode);

        // Mode should be removed from registry if empty
        assert!(registry.is_empty());
    }

    #[test]
    fn test_unregister_for_module_removes_mode_if_empty() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let owner = ModuleId::new("owner");
        let keys = KeySequence::parse("j").unwrap();

        registry.register_at_layer_for_module(
            BindingLayer::Policy,
            &mode,
            keys,
            test_command("down"),
            owner.clone(),
        );

        assert!(!registry.is_empty());

        registry.unregister_for_module(&owner);

        // Mode should be removed from registry if empty
        assert!(registry.is_empty());
    }

    #[test]
    fn test_unregister_for_module_nonexistent() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "j", test_command("down"));

        let nonexistent = ModuleId::new("nonexistent");
        let removed = registry.unregister_for_module(&nonexistent);

        assert_eq!(removed, 0);
        assert_eq!(registry.binding_count(&mode), 1);
    }

    #[test]
    fn test_has_longer_bindings_false() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "j", test_command("down"));

        let j = KeySequence::parse("j").unwrap();
        assert!(!registry.has_longer_bindings(&mode, &j));
    }

    #[test]
    fn test_has_longer_bindings_true() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "gg", test_command("goto-top"));

        let g = KeySequence::parse("g").unwrap();
        assert!(registry.has_longer_bindings(&mode, &g));
    }

    #[test]
    fn test_is_empty_with_empty_entries() {
        let registry = KeymapRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_binding_count_nonexistent_mode() {
        let registry = KeymapRegistry::new();
        let mode = test_mode();
        assert_eq!(registry.binding_count(&mode), 0);
    }

    #[test]
    fn test_layer_sorting() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("x").unwrap();

        // Add in reverse order
        registry.register_at_layer(BindingLayer::Base, &mode, keys.clone(), test_command("base"));
        registry.register_at_layer(BindingLayer::User, &mode, keys.clone(), test_command("user"));
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("policy"),
        );

        // User layer should win
        let binding = registry.get_binding(&mode, &keys);
        assert_eq!(binding, Some(test_command("user")));
    }

    #[test]
    fn test_modes_empty_registry() {
        let registry = KeymapRegistry::new();
        assert_eq!(registry.modes().count(), 0);
    }

    #[test]
    fn test_clear_layer_removes_all_bindings_and_cleans_empty_modes() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let j = KeySequence::parse("j").unwrap();

        registry.register_at_layer(BindingLayer::Policy, &mode, j, test_command("down"));
        assert_eq!(registry.total_bindings(), 1);

        // Clearing the only layer should remove the mode entry entirely
        registry.clear_layer(BindingLayer::Policy, &mode);
        assert_eq!(registry.total_bindings(), 0);
        assert_eq!(registry.modes().count(), 0);
    }

    #[test]
    fn test_clear_layer_nonexistent_mode_is_noop() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        // clear_layer on a mode that doesn't exist should not panic
        registry.clear_layer(BindingLayer::Policy, &mode);
        assert_eq!(registry.total_bindings(), 0);
    }
}
