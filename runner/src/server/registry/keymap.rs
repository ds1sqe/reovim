//! Keymap registry for mapping key sequences to commands.
//!
//! Supports multi-key sequences like `gg`, `<C-w>h`, etc. The registry
//! can distinguish between:
//! - **Full match**: The key sequence maps to a command
//! - **Prefix match**: The sequence is a prefix of one or more bindings
//! - **No match**: The sequence doesn't match anything
//!
//! # Module Ownership
//!
//! Keybindings can be registered with optional module ownership via
//! [`KeymapRegistry::register_for_module`]. When a module is unloaded, all its
//! registered keybindings can be removed via [`KeymapRegistry::unregister_for_module`].

use std::collections::HashMap;

use {
    reovim_driver_input::KeySequence,
    reovim_kernel::{
        api::v1::{CommandId, ModeId, ModuleId},
        profile_scope,
    },
};

/// Result of a keymap lookup.
///
/// Used by the event loop to determine how to handle a key sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyLookupResult {
    /// Full match: the key sequence maps to this command.
    Found(CommandId),

    /// Prefix match: the sequence is a prefix of one or more bindings.
    ///
    /// The event loop should wait for more keys.
    Prefix,

    /// No match: the sequence doesn't match any binding.
    ///
    /// The event loop should delegate to the fallback handler.
    NotFound,
}

impl KeyLookupResult {
    /// Check if this is a full match.
    #[must_use]
    pub const fn is_found(&self) -> bool {
        matches!(self, Self::Found(_))
    }

    /// Check if this is a prefix match.
    #[must_use]
    pub const fn is_prefix(&self) -> bool {
        matches!(self, Self::Prefix)
    }

    /// Check if this is no match.
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    /// Get the command ID if this is a full match.
    #[must_use]
    pub const fn command_id(&self) -> Option<&CommandId> {
        match self {
            Self::Found(id) => Some(id),
            _ => None,
        }
    }
}

/// Entry in the keymap registry with optional ownership tracking.
struct KeybindingEntry {
    /// The command ID to execute.
    command: CommandId,
    /// The module that owns this keybinding (if any).
    owner: Option<ModuleId>,
}

/// Registry for keybindings.
///
/// Maps (mode, key sequence) pairs to command IDs. Supports multi-key
/// sequences with prefix detection for proper handling of sequences
/// like `gg` or `<C-w>h`.
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
/// use reovim_driver_input::KeySequence;
///
/// let mut registry = KeymapRegistry::new();
///
/// // Register `gg` in normal mode
/// let normal_mode = EditorMode::NORMAL_ID;
/// let gg = KeySequence::parse("gg").unwrap();
/// let goto_top = CommandId::new(EDITOR_MODULE, "goto-top");
/// registry.register(normal_mode.clone(), gg, goto_top);
///
/// // Lookup single `g` - should be prefix
/// let g = KeySequence::parse("g").unwrap();
/// assert!(registry.lookup(&normal_mode, &g).is_prefix());
///
/// // Lookup `gg` - should be found
/// let gg = KeySequence::parse("gg").unwrap();
/// assert!(registry.lookup(&normal_mode, &gg).is_found());
/// ```
#[derive(Default)]
pub struct KeymapRegistry {
    /// Bindings organized by mode, then by key sequence.
    entries: HashMap<ModeId, HashMap<KeySequence, KeybindingEntry>>,
}

impl KeymapRegistry {
    /// Create a new empty keymap registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a keybinding (without module ownership).
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode in which this binding is active
    /// * `keys` - The key sequence that triggers the binding
    /// * `command` - The command to execute
    ///
    /// If a binding with the same mode and key sequence already exists,
    /// it is replaced.
    pub fn register(&mut self, mode: ModeId, keys: KeySequence, command: CommandId) {
        self.entries.entry(mode).or_default().insert(
            keys,
            KeybindingEntry {
                command,
                owner: None,
            },
        );
    }

    /// Register a keybinding with module ownership.
    ///
    /// When the owning module is unloaded, this keybinding will be automatically
    /// deregistered via [`Self::unregister_for_module`].
    pub fn register_for_module(
        &mut self,
        mode: ModeId,
        keys: KeySequence,
        command: CommandId,
        owner: ModuleId,
    ) {
        self.entries.entry(mode).or_default().insert(
            keys,
            KeybindingEntry {
                command,
                owner: Some(owner),
            },
        );
    }

    /// Remove all keybindings owned by a module.
    ///
    /// Called when a module is being unloaded to clean up its registrations.
    ///
    /// Returns the number of keybindings that were removed.
    pub fn unregister_for_module(&mut self, module: &ModuleId) -> usize {
        let mut removed = 0;
        for mode_entries in self.entries.values_mut() {
            let before = mode_entries.len();
            mode_entries.retain(|_, entry| entry.owner.as_ref() != Some(module));
            removed += before - mode_entries.len();
        }
        // Clean up empty mode maps
        self.entries
            .retain(|_, mode_entries| !mode_entries.is_empty());
        removed
    }

    /// Register a keybinding from a string.
    ///
    /// Convenience method that parses the key sequence from a string.
    /// Returns `false` if the string couldn't be parsed.
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode in which this binding is active
    /// * `keys` - Key sequence string (e.g., "gg", "`<C-w>`h")
    /// * `command` - The command to execute
    pub fn register_str(&mut self, mode: ModeId, keys: &str, command: CommandId) -> bool {
        KeySequence::parse(keys).is_some_and(|seq| {
            self.register(mode, seq, command);
            true
        })
    }

    /// Look up a key sequence in a mode.
    ///
    /// Returns:
    /// - `Found(cmd)` if the sequence exactly matches a binding AND is NOT a prefix of any longer binding
    /// - `Prefix` if the sequence is a prefix of one or more longer bindings (even if it also matches exactly)
    /// - `NotFound` if the sequence doesn't match anything
    ///
    /// This implements vim-style "wait for more keys" behavior:
    /// - `d` is both an exact match (enter-delete-operator) and a prefix of `dd` (delete-line)
    /// - We return `Prefix` so the user can type `dd` to delete a line
    /// - If the user wanted just `d`, they can press another motion like `dw`
    #[must_use]
    pub fn lookup(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        profile_scope!("keymap_lookup", "runner::keymap");

        let Some(mode_entries) = self.entries.get(mode) else {
            return KeyLookupResult::NotFound;
        };

        let exact_match = mode_entries.get(keys);
        let is_prefix = Self::is_prefix_of_any(mode_entries, keys);

        // Vim-style: if this is a prefix of something longer, wait for more keys
        // even if it's also an exact match (e.g., 'd' is both d-> and dd)
        if is_prefix {
            return KeyLookupResult::Prefix;
        }

        // Not a prefix - return exact match if found
        if let Some(entry) = exact_match {
            return KeyLookupResult::Found(entry.command.clone());
        }

        KeyLookupResult::NotFound
    }

    /// Check if a key sequence is a prefix of any binding in the mode.
    fn is_prefix_of_any(
        mode_entries: &HashMap<KeySequence, KeybindingEntry>,
        keys: &KeySequence,
    ) -> bool {
        mode_entries
            .keys()
            .any(|binding_keys| binding_keys.starts_with(keys) && binding_keys != keys)
    }

    /// Get all bindings for a mode.
    #[must_use]
    pub fn bindings_for_mode(&self, mode: &ModeId) -> Vec<(&KeySequence, &CommandId)> {
        self.entries
            .get(mode)
            .map(|m| m.iter().map(|(k, e)| (k, &e.command)).collect())
            .unwrap_or_default()
    }

    /// Get the number of bindings for a mode.
    #[must_use]
    pub fn binding_count(&self, mode: &ModeId) -> usize {
        self.entries.get(mode).map_or(0, HashMap::len)
    }

    /// Get total number of bindings across all modes.
    #[must_use]
    pub fn total_bindings(&self) -> usize {
        self.entries.values().map(HashMap::len).sum()
    }

    /// Get all modes that have bindings.
    pub fn modes(&self) -> impl Iterator<Item = &ModeId> {
        self.entries.keys()
    }

    /// Check if the registry has any bindings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() || self.entries.values().all(HashMap::is_empty)
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_command(name: &'static str) -> CommandId {
        CommandId::new(ModuleId::new("test"), name)
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

        registry.register(mode.clone(), keys, cmd);

        assert_eq!(registry.binding_count(&mode), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_keymap_registry_register_str() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let cmd = test_command("cursor-down");

        assert!(registry.register_str(mode.clone(), "j", cmd));
        assert_eq!(registry.binding_count(&mode), 1);
    }

    #[test]
    fn test_keymap_registry_lookup_found() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();
        let cmd = test_command("cursor-down");

        registry.register_str(mode.clone(), "j", cmd.clone());

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
        registry.register_str(mode.clone(), "gg", test_command("goto-top"));

        // Look up single `g` - should be prefix
        let g = KeySequence::parse("g").unwrap();
        let result = registry.lookup(&mode, &g);

        assert!(result.is_prefix());
    }

    #[test]
    fn test_keymap_registry_lookup_not_found() {
        let mut registry = KeymapRegistry::new();
        let mode = test_mode();

        registry.register_str(mode.clone(), "j", test_command("cursor-down"));

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
        registry.register_str(mode.clone(), "g", test_command("goto"));
        registry.register_str(mode.clone(), "gg", test_command("goto-top"));

        // Single `g` is a prefix of `gg`, so we return Prefix (vim-style wait for more keys)
        let g = KeySequence::parse("g").unwrap();
        let result = registry.lookup(&mode, &g);
        assert!(result.is_prefix());

        // `gg` is an exact match with no longer prefix
        let gg = KeySequence::parse("gg").unwrap();
        let result = registry.lookup(&mode, &gg);
        assert!(result.is_found());
    }

    #[test]
    fn test_keymap_registry_different_modes() {
        let mut registry = KeymapRegistry::new();
        let normal = ModeId::new(ModuleId::new("test"), "normal");
        let insert = ModeId::new(ModuleId::new("test"), "insert");

        // Same key, different commands in different modes
        registry.register_str(normal.clone(), "j", test_command("cursor-down"));
        registry.register_str(insert.clone(), "j", test_command("insert-j"));

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

        registry.register_str(mode.clone(), "j", test_command("down"));
        registry.register_str(mode.clone(), "k", test_command("up"));

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

        registry.register_for_module(mode.clone(), keys.clone(), test_command("down"), owner);

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
        registry.register_for_module(mode.clone(), j.clone(), test_command("down"), owner.clone());
        registry.register_for_module(mode.clone(), k.clone(), test_command("up"), owner.clone());

        // Register one without owner
        let l = KeySequence::parse("l").unwrap();
        registry.register(mode.clone(), l.clone(), test_command("right"));

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
        let normal_mode = ModeId::new(ModuleId::new("test"), "normal");
        let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
        let owner = ModuleId::new("my-module");

        // Register in multiple modes
        let j = KeySequence::parse("j").unwrap();
        registry.register_for_module(normal_mode, j.clone(), test_command("down"), owner.clone());
        registry.register_for_module(insert_mode, j, test_command("insert-j"), owner.clone());

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
        registry.register(mode.clone(), j, test_command("down"));

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

        registry.register_for_module(
            mode.clone(),
            j.clone(),
            test_command("a-down"),
            module_x.clone(),
        );
        registry.register_for_module(mode.clone(), k.clone(), test_command("b-up"), module_y);

        assert_eq!(registry.total_bindings(), 2);

        // Unload module A
        registry.unregister_for_module(&module_x);

        assert_eq!(registry.total_bindings(), 1);
        assert!(registry.lookup(&mode, &j).is_not_found());
        assert!(registry.lookup(&mode, &k).is_found());
    }
}
