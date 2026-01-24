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
//!
//! # Module Ownership
//!
//! Keybindings can be registered with optional module ownership via
//! [`KeymapRegistry::register_for_module`]. When a module is unloaded, all its
//! registered keybindings can be removed via [`KeymapRegistry::unregister_for_module`].
//!
//! # Mechanism vs Policy
//!
//! The registry is pure mechanism - it reports FACTS via [`Self::query()`].
//! Policy decisions (wait for `dd` vs execute `d` immediately) are made by
//! `ModeKeyResolver` implementations, not the registry.

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
    ///
    /// When true, this entry shadows lower layers without providing a command.
    /// Used by user config to disable default bindings.
    removed: bool,
}

/// Registry for keybindings with layered composition.
///
/// Maps (mode, key sequence) pairs to command IDs. Supports multi-key
/// sequences with prefix detection for proper handling of sequences
/// like `gg` or `<C-w>h`.
///
/// # Layered Bindings
///
/// Bindings are organized into layers (User > Policy > Base). When multiple
/// layers define the same binding, the highest layer wins.
///
/// # Module Ownership
///
/// Keybindings can be registered with module ownership via [`Self::register_for_module`].
/// This enables automatic cleanup when modules are unloaded.
///
/// # Example
///
/// ```ignore
/// use runner::registry::{KeymapRegistry, KeyLookupResult};
/// use reovim_driver_input::{KeySequence, BindingLayer};
///
/// let mut registry = KeymapRegistry::new();
///
/// // Register `d` at Policy layer (Vim default)
/// registry.register_at_layer(BindingLayer::Policy, &normal, keys, cmd("delete"));
///
/// // Register `d` at User layer (user override)
/// registry.register_at_layer(BindingLayer::User, &normal, keys, cmd("custom"));
///
/// // get_binding returns user's binding (higher layer wins)
/// assert_eq!(registry.get_binding(&normal, &keys), Some(cmd("custom")));
///
/// // query() returns facts for ModeKeyResolver to decide behavior
/// let state = registry.query(&normal, &keys);
/// ```
#[derive(Default)]
pub struct KeymapRegistry {
    /// Bindings organized by mode, then by key sequence, then by layer.
    /// Each key sequence can have multiple entries (one per layer).
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
    /// If a binding already exists at the same layer, it is replaced.
    ///
    /// # Arguments
    ///
    /// * `layer` - The binding layer (User > Policy > Base)
    /// * `mode` - The mode in which this binding is active
    /// * `keys` - The key sequence that triggers the binding
    /// * `command` - The command to execute
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
    ///
    /// When the owning module is unloaded, this keybinding will be automatically
    /// deregistered via [`Self::unregister_for_module`].
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

        // Remove existing entry at same layer (if any)
        key_entries.retain(|e| e.layer != layer);

        // Add new entry
        key_entries.push(KeybindingEntry {
            command,
            layer,
            owner: Some(owner),
            removed: false,
        });

        // Sort by layer (highest first)
        key_entries.sort_by_key(|entry| std::cmp::Reverse(entry.layer));
    }

    /// Get the effective binding for a key sequence (highest layer wins).
    ///
    /// Returns `None` if no binding exists at any layer, or if the highest
    /// layer entry is marked as removed.
    #[must_use]
    pub fn get_binding(&self, mode: &ModeId, keys: &KeySequence) -> Option<CommandId> {
        self.entries
            .get(mode)
            .and_then(|m| m.get(keys))
            .and_then(|entries| entries.first()) // Already sorted, first is highest
            .filter(|e| !e.removed) // Skip removed entries
            .map(|e| e.command.clone())
    }

    /// Pure query - reports facts about what exists (no policy decisions).
    ///
    /// This is the core mechanism API. It returns FACTS about what bindings
    /// exist for a key sequence. The `ModeKeyResolver` then applies its
    /// policy to decide what to do with these facts.
    ///
    /// # Returns
    ///
    /// - `ExactOnly(cmd)` - Exact match exists, no longer bindings
    /// - `ExactWithLonger { exact }` - Exact match AND longer bindings exist
    /// - `PrefixOnly` - No exact match, but longer bindings exist
    /// - `NotFound` - Nothing matches
    #[must_use]
    pub fn query(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupState {
        profile_scope!("keymap_query", "runner::keymap");

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
    ///
    /// Returns `true` if there are bindings that start with `keys` but are longer.
    #[must_use]
    pub fn has_longer_bindings(&self, mode: &ModeId, keys: &KeySequence) -> bool {
        self.entries.get(mode).is_some_and(|mode_entries| {
            mode_entries
                .iter()
                .any(|(k, entries)| k.starts_with(keys) && k != keys && !entries.is_empty())
        })
    }

    /// Remove all bindings at a specific layer for a mode.
    ///
    /// Useful for clearing user overrides or reloading a policy module.
    pub fn clear_layer(&mut self, layer: BindingLayer, mode: &ModeId) {
        if let Some(mode_entries) = self.entries.get_mut(mode) {
            for entries in mode_entries.values_mut() {
                entries.retain(|e| e.layer != layer);
            }
            // Clean up empty key entries
            mode_entries.retain(|_, entries| !entries.is_empty());
        }
        // Clean up empty mode entries
        self.entries
            .retain(|_, mode_entries| !mode_entries.is_empty());
    }

    /// Mark a binding as removed at a specific layer.
    ///
    /// This shadows any lower-layer bindings for this key sequence,
    /// effectively disabling the binding. Unlike `clear_layer()`, this
    /// adds a tombstone entry that prevents lookup from falling through
    /// to lower layers.
    ///
    /// # Arguments
    ///
    /// * `layer` - The layer to add the removal marker at (typically User)
    /// * `mode` - The mode in which to remove the binding
    /// * `keys` - The key sequence to disable
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Disable 'Q' in normal mode (user doesn't want ex mode)
    /// registry.remove_at_layer(BindingLayer::User, &normal_mode, q_keys);
    ///
    /// // Now get_binding returns None even if Policy layer has a binding
    /// assert!(registry.get_binding(&normal_mode, &q_keys).is_none());
    /// ```
    pub fn remove_at_layer(&mut self, layer: BindingLayer, mode: &ModeId, keys: KeySequence) {
        let mode_entries = self.entries.entry(mode.clone()).or_default();
        let key_entries = mode_entries.entry(keys).or_default();

        // Remove existing entry at same layer (if any)
        key_entries.retain(|e| e.layer != layer);

        // Add tombstone entry
        // Use a placeholder command (it won't be executed due to removed flag)
        key_entries.push(KeybindingEntry {
            command: CommandId::new(ModuleId::new("system"), "noop"),
            layer,
            owner: None,
            removed: true,
        });

        // Sort by layer (highest first)
        key_entries.sort_by_key(|entry| std::cmp::Reverse(entry.layer));
    }

    // ========================================================================
    // Legacy API (backward compatibility)
    // ========================================================================

    /// Register a keybinding at the Policy layer (without module ownership).
    ///
    /// This is the legacy API. New code should use [`Self::register_at_layer`].
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode in which this binding is active
    /// * `keys` - The key sequence that triggers the binding
    /// * `command` - The command to execute
    pub fn register(&mut self, mode: &ModeId, keys: KeySequence, command: CommandId) {
        self.register_at_layer(BindingLayer::Policy, mode, keys, command);
    }

    /// Register a keybinding at the Policy layer with module ownership.
    ///
    /// This is the legacy API. New code should use [`Self::register_at_layer_for_module`].
    pub fn register_for_module(
        &mut self,
        mode: &ModeId,
        keys: KeySequence,
        command: CommandId,
        owner: ModuleId,
    ) {
        self.register_at_layer_for_module(BindingLayer::Policy, mode, keys, command, owner);
    }

    /// Remove all keybindings owned by a module.
    ///
    /// Called when a module is being unloaded to clean up its registrations.
    ///
    /// Returns the number of keybindings that were removed.
    pub fn unregister_for_module(&mut self, module: &ModuleId) -> usize {
        let mut removed = 0;
        for mode_entries in self.entries.values_mut() {
            for entries in mode_entries.values_mut() {
                let before = entries.len();
                entries.retain(|e| e.owner.as_ref() != Some(module));
                removed += before - entries.len();
            }
            // Clean up empty key entries
            mode_entries.retain(|_, entries| !entries.is_empty());
        }
        // Clean up empty mode maps
        self.entries
            .retain(|_, mode_entries| !mode_entries.is_empty());
        removed
    }

    /// Register a keybinding from a string at the Policy layer.
    ///
    /// Convenience method that parses the key sequence from a string.
    /// Returns `false` if the string couldn't be parsed.
    pub fn register_str(&mut self, mode: &ModeId, keys: &str, command: CommandId) -> bool {
        KeySequence::parse(keys).is_some_and(|seq| {
            self.register(mode, seq, command);
            true
        })
    }

    /// Look up a key sequence in a mode (Vim-style behavior).
    ///
    /// This method uses VIM-STYLE behavior for backward compatibility:
    /// - If longer bindings exist, return `Prefix` (wait for more keys)
    /// - This allows sequences like `dd` to work (waits after first `d`)
    ///
    /// For other policies, use [`Self::lookup_with_policy()`].
    ///
    /// # Returns
    ///
    /// - `Found(cmd)` if an exact match exists AND no longer bindings exist
    /// - `Prefix` if longer bindings exist (even if exact match also exists)
    /// - `NotFound` if nothing matches
    #[must_use]
    pub fn lookup(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        profile_scope!("keymap_lookup", "runner::keymap");
        self.lookup_with_policy(mode, keys, &VimLookupPolicy)
    }

    /// Look up a key sequence in a mode with a specific policy.
    ///
    /// This method allows you to provide a custom `KeyLookupPolicy` that
    /// determines how to interpret the lookup facts.
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode to look up in
    /// * `keys` - The key sequence to look up
    /// * `policy` - The policy that interprets the facts
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_driver_input::{VimLookupPolicy, EagerLookupPolicy};
    ///
    /// // Vim-style: wait for longer sequences
    /// let result = registry.lookup_with_policy(&mode, &keys, &VimLookupPolicy);
    ///
    /// // Eager: execute exact matches immediately
    /// let result = registry.lookup_with_policy(&mode, &keys, &EagerLookupPolicy);
    /// ```
    #[must_use]
    pub fn lookup_with_policy(
        &self,
        mode: &ModeId,
        keys: &KeySequence,
        policy: &dyn KeyLookupPolicy,
    ) -> KeyLookupResult {
        profile_scope!("keymap_lookup_with_policy", "runner::keymap");
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
        // Delegate to the inherent method
        Self::query(self, mode, keys)
    }

    fn has_longer_bindings(&self, mode: &ModeId, keys: &KeySequence) -> bool {
        // Delegate to the inherent method
        Self::has_longer_bindings(self, mode, keys)
    }

    fn get_exact(&self, mode: &ModeId, keys: &KeySequence) -> Option<CommandId> {
        // Delegate to get_binding (which returns effective binding considering layers)
        self.get_binding(mode, keys)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    fn test_mode() -> ModeId {
        ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0)
    }

    fn test_mode_normal() -> ModeId {
        ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0)
    }

    fn test_mode_insert() -> ModeId {
        ModeId::with_discriminant(TEST_MODULE, "INSERT", 1)
    }

    fn test_mode_visual() -> ModeId {
        ModeId::with_discriminant(TEST_MODULE, "VISUAL", 2)
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

        registry.register(&mode, keys, cmd);

        assert_eq!(registry.binding_count(&mode), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_keymap_registry_register_str() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let cmd = test_command("cursor-down");

        assert!(registry.register_str(&mode, "j", cmd));
        assert_eq!(registry.binding_count(&mode), 1);
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
    fn test_keymap_registry_lookup_not_found() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "j", test_command("cursor-down"));

        // Look up unbound key
        let x = KeySequence::parse("x").unwrap();
        let result = registry.lookup(&mode, &x);

        assert!(result.is_not_found());
    }

    #[test]
    fn test_keymap_registry_multi_key_sequence() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Register both `g` and `gg`
        registry.register_str(&mode, "g", test_command("goto"));
        registry.register_str(&mode, "gg", test_command("goto-top"));

        // lookup() uses VIM-STYLE semantics: returns Prefix if longer bindings exist
        // (for backward compatibility with existing behavior)
        let g = KeySequence::parse("g").unwrap();
        let result = registry.lookup(&mode, &g);
        assert!(result.is_prefix()); // VIM: wait for more keys since `gg` exists

        // query() shows the full picture for resolvers to make policy decisions
        let state = registry.query(&mode, &g);
        assert_eq!(
            state,
            KeyLookupState::ExactWithLonger {
                exact: test_command("goto")
            }
        );

        // `gg` is an exact match (and not a prefix of anything longer)
        let gg = KeySequence::parse("gg").unwrap();
        let result = registry.lookup(&mode, &gg);
        assert!(result.is_found());
        assert_eq!(result.command_id().unwrap().name(), "goto-top");
    }

    #[test]
    fn test_keymap_registry_prefix_only() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Register only `gg` (no binding for `g` alone)
        registry.register_str(&mode, "gg", test_command("goto-top"));

        // Single `g` has no exact match but is a prefix of `gg` - return Prefix
        let g = KeySequence::parse("g").unwrap();
        let result = registry.lookup(&mode, &g);
        assert!(result.is_prefix());

        // `gg` is an exact match
        let gg = KeySequence::parse("gg").unwrap();
        let result = registry.lookup(&mode, &gg);
        assert!(result.is_found());
    }

    #[test]
    fn test_keymap_registry_different_modes() {
        let mut registry = KeymapRegistry::new();
        let normal = test_mode_normal();
        let insert = test_mode_insert();

        // Same key, different commands in different modes
        registry.register_str(&normal, "j", test_command("cursor-down"));
        registry.register_str(&insert, "j", test_command("insert-j"));

        let j = KeySequence::parse("j").unwrap();

        // In normal mode
        let result = registry.lookup(&normal, &j);
        assert_eq!(result.command_id().unwrap().name(), "cursor-down");

        // In insert mode
        let result = registry.lookup(&insert, &j);
        assert_eq!(result.command_id().unwrap().name(), "insert-j");
    }

    #[test]
    fn test_keymap_registry_bindings_for_mode() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "j", test_command("down"));
        registry.register_str(&mode, "k", test_command("up"));

        let bindings = registry.bindings_for_mode(&mode);
        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn test_key_lookup_result_methods() {
        let found = KeyLookupResult::Found(test_command("test"));
        assert!(found.is_found());
        assert!(!found.is_prefix());
        assert!(!found.is_not_found());
        assert!(found.command_id().is_some());

        let prefix = KeyLookupResult::Prefix;
        assert!(!prefix.is_found());
        assert!(prefix.is_prefix());
        assert!(!prefix.is_not_found());
        assert!(prefix.command_id().is_none());

        let not_found = KeyLookupResult::NotFound;
        assert!(!not_found.is_found());
        assert!(!not_found.is_prefix());
        assert!(not_found.is_not_found());
        assert!(not_found.command_id().is_none());
    }

    #[test]
    fn test_keymap_registry_register_for_module() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let owner = ModuleId::new("my-module");
        let keys = KeySequence::parse("j").unwrap();

        registry.register_for_module(&mode, keys.clone(), test_command("down"), owner);

        assert_eq!(registry.binding_count(&mode), 1);

        let result = registry.lookup(&mode, &keys);
        assert!(result.is_found());
    }

    #[test]
    fn test_keymap_registry_unregister_for_module() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let owner = ModuleId::new("my-module");

        // Register keybindings for the module
        let j = KeySequence::parse("j").unwrap();
        let k = KeySequence::parse("k").unwrap();
        registry.register_for_module(&mode, j.clone(), test_command("down"), owner.clone());
        registry.register_for_module(&mode, k.clone(), test_command("up"), owner.clone());

        // Register one without owner
        let l = KeySequence::parse("l").unwrap();
        registry.register(&mode, l.clone(), test_command("right"));

        assert_eq!(registry.binding_count(&mode), 3);

        // Unregister module's keybindings
        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 2);
        assert_eq!(registry.binding_count(&mode), 1);

        // Module keybindings should be gone
        assert!(registry.lookup(&mode, &j).is_not_found());
        assert!(registry.lookup(&mode, &k).is_not_found());

        // Non-owned keybinding should remain
        assert!(registry.lookup(&mode, &l).is_found());
    }

    #[test]
    fn test_keymap_registry_unregister_for_module_multiple_modes() {
        let mut registry = KeymapRegistry::new();
        let normal_mode = test_mode_normal();
        let insert_mode = test_mode_insert();
        let owner = ModuleId::new("my-module");

        // Register in multiple modes
        let j = KeySequence::parse("j").unwrap();
        registry.register_for_module(&normal_mode, j.clone(), test_command("down"), owner.clone());
        registry.register_for_module(&insert_mode, j, test_command("insert-j"), owner.clone());

        assert_eq!(registry.total_bindings(), 2);

        // Unregister all module keybindings
        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 2);
        assert_eq!(registry.total_bindings(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_keymap_registry_unregister_for_module_empty() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let owner = ModuleId::new("my-module");

        // Register without owner
        let j = KeySequence::parse("j").unwrap();
        registry.register(&mode, j, test_command("down"));

        // Try to unregister for a module that has no keybindings
        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 0);
        assert_eq!(registry.binding_count(&mode), 1);
    }

    #[test]
    fn test_keymap_registry_multiple_modules() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let module_x = ModuleId::new("module-a");
        let module_y = ModuleId::new("module-b");

        let j = KeySequence::parse("j").unwrap();
        let k = KeySequence::parse("k").unwrap();

        registry.register_for_module(&mode, j.clone(), test_command("a-down"), module_x.clone());
        registry.register_for_module(&mode, k.clone(), test_command("b-up"), module_y);

        assert_eq!(registry.total_bindings(), 2);

        // Unload module A
        registry.unregister_for_module(&module_x);

        assert_eq!(registry.total_bindings(), 1);
        assert!(registry.lookup(&mode, &j).is_not_found());
        assert!(registry.lookup(&mode, &k).is_found());
    }

    // ========================================================================
    // Phase 1 Tests: KeyLookupState Variants (Epic #353)
    // ========================================================================

    #[test]
    fn test_query_exact_only() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Register 'x' only (no 'xx', 'xy', etc.)
        registry.register_str(&mode, "x", test_command("delete-char"));

        let x = KeySequence::parse("x").unwrap();
        let state = registry.query(&mode, &x);

        assert_eq!(state, KeyLookupState::ExactOnly(test_command("delete-char")));
    }

    #[test]
    fn test_query_exact_with_longer() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Register 'd' and 'dd' - classic Vim scenario
        registry.register_str(&mode, "d", test_command("delete"));
        registry.register_str(&mode, "dd", test_command("delete-line"));

        let d = KeySequence::parse("d").unwrap();
        let state = registry.query(&mode, &d);

        // 'd' has exact match AND 'dd' exists (longer binding)
        assert_eq!(
            state,
            KeyLookupState::ExactWithLonger {
                exact: test_command("delete")
            }
        );
    }

    #[test]
    fn test_query_prefix_only() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Register only 'gg' (no binding for 'g' alone)
        registry.register_str(&mode, "gg", test_command("goto-top"));

        let g = KeySequence::parse("g").unwrap();
        let state = registry.query(&mode, &g);

        // 'g' has no exact match, but 'gg' exists (prefix)
        assert_eq!(state, KeyLookupState::PrefixOnly);
    }

    #[test]
    fn test_query_not_found() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "j", test_command("cursor-down"));

        // 'z' is not registered and not a prefix of anything
        let z = KeySequence::parse("z").unwrap();
        let state = registry.query(&mode, &z);

        assert_eq!(state, KeyLookupState::NotFound);
    }

    #[test]
    fn test_query_empty_registry() {
        let registry = KeymapRegistry::new();
        let mode = test_mode();

        let j = KeySequence::parse("j").unwrap();
        let state = registry.query(&mode, &j);

        assert_eq!(state, KeyLookupState::NotFound);
    }

    #[test]
    fn test_query_unknown_mode() {
        let mut registry = KeymapRegistry::new();
        let normal = test_mode_normal();
        let visual = test_mode_visual();

        // Register in normal mode only
        registry.register_str(&normal, "j", test_command("cursor-down"));

        // Query in visual mode - should be NotFound
        let j = KeySequence::parse("j").unwrap();
        let state = registry.query(&visual, &j);

        assert_eq!(state, KeyLookupState::NotFound);
    }

    // ========================================================================
    // Phase 1 Tests: Layered Bindings (Epic #353)
    // ========================================================================

    #[test]
    fn test_layer_override_policy_with_user() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("d").unwrap();

        // Policy layer: d → delete
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("delete"),
        );

        // User layer: d → custom-delete (overrides policy)
        registry.register_at_layer(
            BindingLayer::User,
            &mode,
            keys.clone(),
            test_command("custom-delete"),
        );

        // get_binding returns user's binding (higher layer wins)
        let binding = registry.get_binding(&mode, &keys);
        assert_eq!(binding, Some(test_command("custom-delete")));
    }

    #[test]
    fn test_layer_fallback_to_policy() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("j").unwrap();

        // Only Policy layer has the binding
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("cursor-down"),
        );

        // get_binding falls back to Policy when User is empty
        let binding = registry.get_binding(&mode, &keys);
        assert_eq!(binding, Some(test_command("cursor-down")));
    }

    #[test]
    fn test_layer_base_fallback() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("h").unwrap();

        // Only Base layer has the binding
        registry.register_at_layer(
            BindingLayer::Base,
            &mode,
            keys.clone(),
            test_command("cursor-left"),
        );

        // get_binding falls back to Base
        let binding = registry.get_binding(&mode, &keys);
        assert_eq!(binding, Some(test_command("cursor-left")));
    }

    #[test]
    fn test_clear_user_layer() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("d").unwrap();

        // Policy layer: d → delete
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("delete"),
        );

        // User layer: d → custom-delete
        registry.register_at_layer(
            BindingLayer::User,
            &mode,
            keys.clone(),
            test_command("custom-delete"),
        );

        // Before clear: user layer wins
        assert_eq!(registry.get_binding(&mode, &keys), Some(test_command("custom-delete")));

        // Clear user layer
        registry.clear_layer(BindingLayer::User, &mode);

        // After clear: falls back to policy layer
        assert_eq!(registry.get_binding(&mode, &keys), Some(test_command("delete")));
    }

    #[test]
    fn test_query_sees_effective_binding() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Policy layer: d → delete, dd → delete-line
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            KeySequence::parse("d").unwrap(),
            test_command("delete"),
        );
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            KeySequence::parse("dd").unwrap(),
            test_command("delete-line"),
        );

        // User layer: d → custom-delete (overrides policy 'd')
        registry.register_at_layer(
            BindingLayer::User,
            &mode,
            KeySequence::parse("d").unwrap(),
            test_command("custom-delete"),
        );

        // query() should see user's 'd' binding with 'dd' still existing
        let d = KeySequence::parse("d").unwrap();
        let state = registry.query(&mode, &d);

        assert_eq!(
            state,
            KeyLookupState::ExactWithLonger {
                exact: test_command("custom-delete")
            }
        );
    }

    #[test]
    fn test_register_same_key_same_layer_replaces() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("j").unwrap();

        // First registration at Policy layer
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("old-down"),
        );

        // Second registration at same layer - should replace
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("new-down"),
        );

        // Only one binding should exist
        assert_eq!(registry.binding_count(&mode), 1);

        // The new command should be active
        assert_eq!(registry.get_binding(&mode, &keys), Some(test_command("new-down")));
    }

    #[test]
    fn test_layer_ordering_enforced() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("j").unwrap();

        // Register in reverse priority order to test sorting
        registry.register_at_layer(
            BindingLayer::User, // Highest
            &mode,
            keys.clone(),
            test_command("user-cmd"),
        );
        registry.register_at_layer(
            BindingLayer::Base, // Lowest
            &mode,
            keys.clone(),
            test_command("base-cmd"),
        );
        registry.register_at_layer(
            BindingLayer::Policy, // Middle
            &mode,
            keys.clone(),
            test_command("policy-cmd"),
        );

        // User layer should win regardless of registration order
        assert_eq!(registry.get_binding(&mode, &keys), Some(test_command("user-cmd")));
    }

    #[test]
    fn test_multi_key_sequence_with_layers() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        // Policy layer: <C-w>h → window-left
        let ctrl_w_h = KeySequence::parse("<C-w>h").unwrap();
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            ctrl_w_h.clone(),
            test_command("window-left"),
        );

        // User layer: <C-w>h → custom-window-left
        registry.register_at_layer(
            BindingLayer::User,
            &mode,
            ctrl_w_h.clone(),
            test_command("custom-window-left"),
        );

        // User layer wins for complex sequences too
        assert_eq!(
            registry.get_binding(&mode, &ctrl_w_h),
            Some(test_command("custom-window-left"))
        );

        // <C-w> alone is prefix
        let ctrl_w = KeySequence::parse("<C-w>").unwrap();
        let state = registry.query(&mode, &ctrl_w);
        assert_eq!(state, KeyLookupState::PrefixOnly);
    }

    #[test]
    fn test_bindings_isolated_per_mode() {
        let mut registry = KeymapRegistry::new();
        let normal = test_mode_normal();
        let insert = test_mode_insert();
        let keys = KeySequence::parse("j").unwrap();

        // Register at different layers in different modes
        registry.register_at_layer(
            BindingLayer::User,
            &normal,
            keys.clone(),
            test_command("normal-j"),
        );
        registry.register_at_layer(
            BindingLayer::Policy,
            &insert,
            keys.clone(),
            test_command("insert-j"),
        );

        // Bindings are isolated per mode
        assert_eq!(registry.get_binding(&normal, &keys), Some(test_command("normal-j")));
        assert_eq!(registry.get_binding(&insert, &keys), Some(test_command("insert-j")));
    }

    #[test]
    fn test_has_longer_bindings() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(&mode, "g", test_command("goto"));
        registry.register_str(&mode, "gg", test_command("goto-top"));
        registry.register_str(&mode, "gj", test_command("goto-down"));

        let g = KeySequence::parse("g").unwrap();
        let gg = KeySequence::parse("gg").unwrap();
        let x = KeySequence::parse("x").unwrap();

        // 'g' has longer bindings (gg, gj)
        assert!(registry.has_longer_bindings(&mode, &g));

        // 'gg' has no longer bindings (no 'ggg', etc.)
        assert!(!registry.has_longer_bindings(&mode, &gg));

        // 'x' is not even registered - no longer bindings
        assert!(!registry.has_longer_bindings(&mode, &x));
    }

    // ========================================================================
    // remove_at_layer tests
    // ========================================================================

    #[test]
    fn test_remove_at_layer_shadows_lower_layer() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("Q").unwrap();

        // Policy layer: Q → ex-mode
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("ex-mode"),
        );

        // Verify binding exists
        assert_eq!(registry.get_binding(&mode, &keys), Some(test_command("ex-mode")));

        // User removes Q at User layer
        registry.remove_at_layer(BindingLayer::User, &mode, keys.clone());

        // Now get_binding returns None (removed entry shadows policy)
        assert!(registry.get_binding(&mode, &keys).is_none());
    }

    #[test]
    fn test_remove_at_layer_does_not_affect_other_keys() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let q = KeySequence::parse("Q").unwrap();
        let j = KeySequence::parse("j").unwrap();

        // Register Q and j at Policy layer
        registry.register_at_layer(BindingLayer::Policy, &mode, q.clone(), test_command("ex-mode"));
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            j.clone(),
            test_command("cursor-down"),
        );

        // Remove only Q
        registry.remove_at_layer(BindingLayer::User, &mode, q.clone());

        // Q is removed, j still works
        assert!(registry.get_binding(&mode, &q).is_none());
        assert_eq!(registry.get_binding(&mode, &j), Some(test_command("cursor-down")));
    }

    #[test]
    fn test_remove_at_layer_clear_restores() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("Q").unwrap();

        // Policy layer: Q → ex-mode
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            test_command("ex-mode"),
        );

        // User removes Q
        registry.remove_at_layer(BindingLayer::User, &mode, keys.clone());
        assert!(registry.get_binding(&mode, &keys).is_none());

        // Clear user layer restores policy binding
        registry.clear_layer(BindingLayer::User, &mode);
        assert_eq!(registry.get_binding(&mode, &keys), Some(test_command("ex-mode")));
    }

    #[test]
    fn test_remove_nonexistent_binding() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let keys = KeySequence::parse("Z").unwrap();

        // Remove a key that was never registered
        registry.remove_at_layer(BindingLayer::User, &mode, keys.clone());

        // Should still return None (no binding to shadow)
        assert!(registry.get_binding(&mode, &keys).is_none());
    }
}
