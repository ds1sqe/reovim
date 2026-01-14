//! Keymap registry for mapping key sequences to commands.
//!
//! Supports multi-key sequences like `gg`, `<C-w>h`, etc. The registry
//! can distinguish between:
//! - **Full match**: The key sequence maps to a command
//! - **Prefix match**: The sequence is a prefix of one or more bindings
//! - **No match**: The sequence doesn't match anything

use std::collections::HashMap;

use {
    reovim_driver_input::KeySequence,
    reovim_kernel::api::v1::{CommandId, ModeId},
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

/// Registry for keybindings.
///
/// Maps (mode, key sequence) pairs to command IDs. Supports multi-key
/// sequences with prefix detection for proper handling of sequences
/// like `gg` or `<C-w>h`.
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
    bindings: HashMap<ModeId, HashMap<KeySequence, CommandId>>,
}

impl KeymapRegistry {
    /// Create a new empty keymap registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a keybinding.
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
        self.bindings.entry(mode).or_default().insert(keys, command);
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
    /// - `Found(cmd)` if the sequence exactly matches a binding
    /// - `Prefix` if the sequence is a prefix of one or more bindings
    /// - `NotFound` if the sequence doesn't match anything
    #[must_use]
    pub fn lookup(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        let Some(mode_bindings) = self.bindings.get(mode) else {
            return KeyLookupResult::NotFound;
        };

        // Check for exact match first
        if let Some(cmd) = mode_bindings.get(keys) {
            return KeyLookupResult::Found(cmd.clone());
        }

        // Check if this is a prefix of any binding
        if Self::is_prefix_of_any(mode_bindings, keys) {
            return KeyLookupResult::Prefix;
        }

        KeyLookupResult::NotFound
    }

    /// Check if a key sequence is a prefix of any binding in the mode.
    fn is_prefix_of_any(
        mode_bindings: &HashMap<KeySequence, CommandId>,
        keys: &KeySequence,
    ) -> bool {
        mode_bindings
            .keys()
            .any(|binding_keys| binding_keys.starts_with(keys) && binding_keys != keys)
    }

    /// Get all bindings for a mode.
    #[must_use]
    pub fn bindings_for_mode(&self, mode: &ModeId) -> Vec<(&KeySequence, &CommandId)> {
        self.bindings
            .get(mode)
            .map(|m| m.iter().collect())
            .unwrap_or_default()
    }

    /// Get the number of bindings for a mode.
    #[must_use]
    pub fn binding_count(&self, mode: &ModeId) -> usize {
        self.bindings.get(mode).map_or(0, HashMap::len)
    }

    /// Get total number of bindings across all modes.
    #[must_use]
    pub fn total_bindings(&self) -> usize {
        self.bindings.values().map(HashMap::len).sum()
    }

    /// Get all modes that have bindings.
    pub fn modes(&self) -> impl Iterator<Item = &ModeId> {
        self.bindings.keys()
    }

    /// Check if the registry has any bindings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty() || self.bindings.values().all(HashMap::is_empty)
    }
}

impl std::fmt::Debug for KeymapRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeymapRegistry")
            .field("modes", &self.bindings.keys().collect::<Vec<_>>())
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

        // Single `g` is an exact match (not prefix)
        let g = KeySequence::parse("g").unwrap();
        let result = registry.lookup(&mode, &g);
        assert!(result.is_found());

        // `gg` is also an exact match
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
}
