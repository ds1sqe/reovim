//! Interactor configuration for input routing policy.
//!
//! This module provides metadata-driven configuration for interactor components,
//! allowing them to declare their input handling behavior instead of relying on
//! hardcoded checks in the mode system.
//!
//! # Input Routing Policy
//!
//! When a component gains focus (becomes the "interactor"), the keymap module
//! needs to know how to route input:
//!
//! - **Accepting character input**: Single printable characters are routed as
//!   text input (like Insert mode). Used by Explorer's filter mode, Telescope, etc.
//!
//! - **Using keymap**: Characters are looked up in the keymap and dispatched as
//!   commands (like Normal mode). Used by Window mode where `h/j/k/l` navigate.
//!
//! # Example
//!
//! ```
//! use reovim_module_keymap::{InteractorConfig, InteractorRegistry, ComponentId};
//!
//! let mut registry = InteractorRegistry::new();
//!
//! // Register Window mode as using keymap (h/j/k/l navigation)
//! registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
//!
//! // Check if a component accepts character input
//! assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
//!
//! // Unregistered components default to accepting character input
//! let unknown = ComponentId::custom("unknown");
//! assert!(registry.accepts_char_input(&unknown));
//! ```

use std::collections::HashMap;

/// Unique identifier for interactor components.
///
/// Components can be built-in (Editor, Window, `CommandLine`) or custom (plugins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(&'static str);

impl ComponentId {
    /// Editor component (main editing area).
    pub const EDITOR: Self = Self("editor");
    /// Window management mode (Ctrl-W prefix).
    pub const WINDOW: Self = Self("window");
    /// Command line (: prefix).
    pub const COMMAND_LINE: Self = Self("command_line");
    /// Explorer file browser.
    pub const EXPLORER: Self = Self("explorer");
    /// Telescope fuzzy finder.
    pub const TELESCOPE: Self = Self("telescope");

    /// Create a new component ID.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Create a custom component ID.
    #[must_use]
    pub const fn custom(id: &'static str) -> Self {
        Self(id)
    }

    /// Get the string identifier.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

/// Configuration for an interactor component.
///
/// Defines how an interactor handles input. Components that accept character input
/// (like Explorer's filter mode) will receive printable characters directly.
/// Components that use keymap (like Window mode) will have characters looked up
/// in the keymap instead.
#[derive(Debug, Clone, Copy)]
pub struct InteractorConfig {
    /// Whether this interactor accepts character input (insert-mode style).
    ///
    /// When `true`, single printable characters are routed as text input.
    /// When `false`, characters are looked up in the keymap.
    pub accepts_char_input: bool,
}

impl InteractorConfig {
    /// Create config that accepts character input.
    ///
    /// Use this for interactors that need text input, like:
    /// - Explorer's create/rename/filter modes
    /// - Telescope's search input
    /// - Leap/jump search modes
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_module_keymap::InteractorConfig;
    ///
    /// let config = InteractorConfig::accepting_input();
    /// assert!(config.accepts_char_input);
    /// ```
    #[must_use]
    pub const fn accepting_input() -> Self {
        Self {
            accepts_char_input: true,
        }
    }

    /// Create config that uses keymap.
    ///
    /// Use this for interactors where keys trigger commands:
    /// - Window mode (h/j/k/l for navigation)
    /// - Any mode where characters map to actions
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_module_keymap::InteractorConfig;
    ///
    /// let config = InteractorConfig::using_keymap();
    /// assert!(!config.accepts_char_input);
    /// ```
    #[must_use]
    pub const fn using_keymap() -> Self {
        Self {
            accepts_char_input: false,
        }
    }
}

impl Default for InteractorConfig {
    fn default() -> Self {
        // Default: accept char input (matches current behavior for most plugins)
        Self::accepting_input()
    }
}

/// Registry for interactor configurations.
///
/// Stores metadata for interactor components, allowing the mode system to
/// query behavior without hardcoding component-specific logic.
///
/// # Default Behavior
///
/// Unregistered components default to accepting character input. This is the
/// safe default that matches the behavior of most plugins.
#[derive(Debug, Default)]
pub struct InteractorRegistry {
    configs: HashMap<ComponentId, InteractorConfig>,
}

impl InteractorRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with built-in configurations.
    ///
    /// Registers the built-in components with their default configurations:
    /// - `WINDOW`: uses keymap (for h/j/k/l navigation)
    /// - Others: accept char input by default
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtins();
        registry
    }

    /// Register configuration for an interactor.
    ///
    /// If the interactor was already registered, the config is replaced.
    pub fn register(&mut self, id: ComponentId, config: InteractorConfig) {
        self.configs.insert(id, config);
    }

    /// Unregister an interactor.
    ///
    /// After unregistration, the component will use the default behavior.
    pub fn unregister(&mut self, id: &ComponentId) {
        self.configs.remove(id);
    }

    /// Get configuration for an interactor.
    ///
    /// Returns `None` if the interactor is not registered.
    #[must_use]
    pub fn get(&self, id: &ComponentId) -> Option<&InteractorConfig> {
        self.configs.get(id)
    }

    /// Check if interactor accepts character input.
    ///
    /// Returns `true` by default for unregistered interactors (safe default
    /// that matches the behavior of most plugins).
    #[must_use]
    pub fn accepts_char_input(&self, id: &ComponentId) -> bool {
        self.get(id).is_none_or(|config| config.accepts_char_input)
    }

    /// Check if interactor uses keymap.
    ///
    /// Returns `false` by default for unregistered interactors.
    #[must_use]
    pub fn uses_keymap(&self, id: &ComponentId) -> bool {
        !self.accepts_char_input(id)
    }

    /// Register all built-in interactor configs.
    ///
    /// This should be called during runtime initialization to set up
    /// core component configurations.
    pub fn register_builtins(&mut self) {
        // Window mode does NOT accept char input - uses keymap for h/j/k/l
        self.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    }

    /// Get the number of registered interactors.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // HashMap::len() is not const
    pub fn len(&self) -> usize {
        self.configs.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // HashMap::is_empty() is not const
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ComponentId: constants
    // ========================================================================

    #[test]
    fn test_component_id_editor_constant() {
        assert_eq!(ComponentId::EDITOR.as_str(), "editor");
    }

    #[test]
    fn test_component_id_window_constant() {
        assert_eq!(ComponentId::WINDOW.as_str(), "window");
    }

    #[test]
    fn test_component_id_command_line_constant() {
        assert_eq!(ComponentId::COMMAND_LINE.as_str(), "command_line");
    }

    #[test]
    fn test_component_id_explorer_constant() {
        assert_eq!(ComponentId::EXPLORER.as_str(), "explorer");
    }

    #[test]
    fn test_component_id_telescope_constant() {
        assert_eq!(ComponentId::TELESCOPE.as_str(), "telescope");
    }

    // ========================================================================
    // ComponentId: construction
    // ========================================================================

    #[test]
    fn test_component_id_new() {
        let id = ComponentId::new("my_component");
        assert_eq!(id.as_str(), "my_component");
    }

    #[test]
    fn test_component_id_custom() {
        let custom = ComponentId::custom("my_plugin");
        assert_eq!(custom.as_str(), "my_plugin");
    }

    #[test]
    fn test_component_id_new_and_custom_are_equivalent() {
        let from_new = ComponentId::new("test_id");
        let from_custom = ComponentId::custom("test_id");
        assert_eq!(from_new, from_custom);
        assert_eq!(from_new.as_str(), from_custom.as_str());
    }

    #[test]
    fn test_component_id_new_matches_constant() {
        assert_eq!(ComponentId::EDITOR, ComponentId::new("editor"));
        assert_eq!(ComponentId::WINDOW, ComponentId::new("window"));
        assert_eq!(ComponentId::COMMAND_LINE, ComponentId::new("command_line"));
        assert_eq!(ComponentId::EXPLORER, ComponentId::new("explorer"));
        assert_eq!(ComponentId::TELESCOPE, ComponentId::new("telescope"));
    }

    #[test]
    fn test_component_id_empty_string() {
        let id = ComponentId::new("");
        assert_eq!(id.as_str(), "");
    }

    // ========================================================================
    // ComponentId: equality and inequality
    // ========================================================================

    #[test]
    fn test_component_id_equality_same() {
        assert_eq!(ComponentId::EDITOR, ComponentId::new("editor"));
    }

    #[test]
    fn test_component_id_inequality_different() {
        assert_ne!(ComponentId::EDITOR, ComponentId::WINDOW);
    }

    #[test]
    fn test_component_id_all_constants_distinct() {
        let ids = [
            ComponentId::EDITOR,
            ComponentId::WINDOW,
            ComponentId::COMMAND_LINE,
            ComponentId::EXPLORER,
            ComponentId::TELESCOPE,
        ];
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "Constants at indices {i} and {j} must be different");
            }
        }
    }

    // ========================================================================
    // ComponentId: trait implementations (Debug, Clone, Copy, Hash)
    // ========================================================================

    #[test]
    fn test_component_id_debug() {
        let id = ComponentId::EDITOR;
        let debug = format!("{id:?}");
        assert!(debug.contains("editor"), "Debug output should contain the id string");
    }

    #[test]
    fn test_component_id_clone() {
        let original = ComponentId::EDITOR;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_component_id_copy() {
        let original = ComponentId::WINDOW;
        let copied = original;
        // Both should still be valid (Copy semantics)
        assert_eq!(original, copied);
        assert_eq!(original.as_str(), "window");
        assert_eq!(copied.as_str(), "window");
    }

    #[test]
    fn test_component_id_hash_in_map() {
        let mut map = HashMap::new();
        map.insert(ComponentId::EDITOR, "editor_value");
        map.insert(ComponentId::WINDOW, "window_value");

        assert_eq!(map.get(&ComponentId::EDITOR), Some(&"editor_value"));
        assert_eq!(map.get(&ComponentId::WINDOW), Some(&"window_value"));
        assert_eq!(map.get(&ComponentId::custom("missing")), None);
    }

    #[test]
    fn test_component_id_hash_consistency() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ComponentId::EDITOR);
        set.insert(ComponentId::new("editor")); // Same value

        assert_eq!(set.len(), 1, "Same component id should hash to same bucket");
    }

    // ========================================================================
    // InteractorConfig: constructors
    // ========================================================================

    #[test]
    fn test_interactor_config_accepting_input() {
        let config = InteractorConfig::accepting_input();
        assert!(config.accepts_char_input);
    }

    #[test]
    fn test_interactor_config_using_keymap() {
        let config = InteractorConfig::using_keymap();
        assert!(!config.accepts_char_input);
    }

    #[test]
    fn test_interactor_config_default() {
        let config = InteractorConfig::default();
        assert!(config.accepts_char_input, "Default should accept char input");
    }

    #[test]
    fn test_interactor_config_default_matches_accepting_input() {
        let default = InteractorConfig::default();
        let accepting = InteractorConfig::accepting_input();
        assert_eq!(default.accepts_char_input, accepting.accepts_char_input);
    }

    // ========================================================================
    // InteractorConfig: trait implementations
    // ========================================================================

    #[test]
    fn test_interactor_config_debug() {
        let config = InteractorConfig::accepting_input();
        let debug = format!("{config:?}");
        assert!(debug.contains("InteractorConfig"), "Debug output should contain type name");
        assert!(debug.contains("accepts_char_input"), "Debug output should contain field name");
    }

    #[test]
    fn test_interactor_config_clone() {
        let original = InteractorConfig::using_keymap();
        let cloned = original;
        assert_eq!(original.accepts_char_input, cloned.accepts_char_input);
    }

    #[test]
    fn test_interactor_config_copy() {
        let original = InteractorConfig::accepting_input();
        let copied = original;
        // Both should still be valid (Copy semantics)
        assert!(original.accepts_char_input);
        assert!(copied.accepts_char_input);
    }

    // ========================================================================
    // InteractorConfig: field access
    // ========================================================================

    #[test]
    fn test_interactor_config_field_direct_access() {
        let mut config = InteractorConfig::accepting_input();
        assert!(config.accepts_char_input);

        config.accepts_char_input = false;
        assert!(!config.accepts_char_input);
    }

    // ========================================================================
    // InteractorRegistry: construction
    // ========================================================================

    #[test]
    fn test_registry_new_is_empty() {
        let registry = InteractorRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = InteractorRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_new_and_default_equivalent() {
        let r1 = InteractorRegistry::new();
        let r2 = InteractorRegistry::default();
        assert_eq!(r1.len(), r2.len());
        assert_eq!(r1.is_empty(), r2.is_empty());
    }

    #[test]
    fn test_registry_with_builtins_not_empty() {
        let registry = InteractorRegistry::with_builtins();
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_with_builtins_has_window() {
        let registry = InteractorRegistry::with_builtins();
        assert!(registry.get(&ComponentId::WINDOW).is_some());
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
    }

    #[test]
    fn test_registry_with_builtins_len() {
        let registry = InteractorRegistry::with_builtins();
        // Currently only WINDOW is registered as builtin
        assert_eq!(registry.len(), 1);
    }

    // ========================================================================
    // InteractorRegistry: register
    // ========================================================================

    #[test]
    fn test_registry_register_single() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_register_multiple() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        registry.register(ComponentId::TELESCOPE, InteractorConfig::accepting_input());
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn test_registry_register_replaces_existing() {
        let mut registry = InteractorRegistry::new();

        // Register as using_keymap
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));

        // Replace with accepting_input
        registry.register(ComponentId::WINDOW, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&ComponentId::WINDOW));

        // Length should still be 1 (replaced, not added)
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_register_custom_component() {
        let mut registry = InteractorRegistry::new();
        let custom = ComponentId::custom("my_plugin_panel");
        registry.register(custom, InteractorConfig::using_keymap());
        assert!(registry.uses_keymap(&custom));
    }

    // ========================================================================
    // InteractorRegistry: register_builtins
    // ========================================================================

    #[test]
    fn test_registry_register_builtins() {
        let mut registry = InteractorRegistry::new();
        assert!(registry.is_empty());

        registry.register_builtins();
        assert!(!registry.is_empty());
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
    }

    #[test]
    fn test_registry_register_builtins_idempotent() {
        let mut registry = InteractorRegistry::new();
        registry.register_builtins();
        let len_after_first = registry.len();

        registry.register_builtins();
        let len_after_second = registry.len();

        // Calling twice should not change the count (HashMap::insert replaces)
        assert_eq!(len_after_first, len_after_second);
    }

    // ========================================================================
    // InteractorRegistry: unregister
    // ========================================================================

    #[test]
    fn test_registry_unregister_existing() {
        let mut registry = InteractorRegistry::with_builtins();
        let initial_len = registry.len();

        registry.unregister(&ComponentId::WINDOW);
        assert_eq!(registry.len(), initial_len - 1);
        assert!(registry.get(&ComponentId::WINDOW).is_none());
    }

    #[test]
    fn test_registry_unregister_nonexistent_is_noop() {
        let mut registry = InteractorRegistry::new();
        let unknown = ComponentId::custom("nonexistent");

        // Should not panic
        registry.unregister(&unknown);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_unregister_restores_default_behavior() {
        let mut registry = InteractorRegistry::with_builtins();

        // WINDOW uses keymap (registered)
        assert!(registry.uses_keymap(&ComponentId::WINDOW));

        registry.unregister(&ComponentId::WINDOW);

        // After unregister, defaults to accepting char input
        assert!(registry.accepts_char_input(&ComponentId::WINDOW));
        assert!(!registry.uses_keymap(&ComponentId::WINDOW));
    }

    #[test]
    fn test_registry_unregister_then_reregister() {
        let mut registry = InteractorRegistry::new();

        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&ComponentId::EXPLORER));

        registry.unregister(&ComponentId::EXPLORER);
        assert!(registry.is_empty());

        // Re-register with different config
        registry.register(ComponentId::EXPLORER, InteractorConfig::using_keymap());
        assert!(registry.uses_keymap(&ComponentId::EXPLORER));
        assert_eq!(registry.len(), 1);
    }

    // ========================================================================
    // InteractorRegistry: get
    // ========================================================================

    #[test]
    fn test_registry_get_registered() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());

        let config = registry.get(&ComponentId::WINDOW);
        assert!(config.is_some());
        assert!(!config.expect("should be registered").accepts_char_input);
    }

    #[test]
    fn test_registry_get_unregistered() {
        let registry = InteractorRegistry::new();
        assert!(registry.get(&ComponentId::EDITOR).is_none());
    }

    #[test]
    fn test_registry_get_after_unregister() {
        let mut registry = InteractorRegistry::with_builtins();
        assert!(registry.get(&ComponentId::WINDOW).is_some());

        registry.unregister(&ComponentId::WINDOW);
        assert!(registry.get(&ComponentId::WINDOW).is_none());
    }

    // ========================================================================
    // InteractorRegistry: accepts_char_input
    // ========================================================================

    #[test]
    fn test_registry_accepts_char_input_unregistered_defaults_true() {
        let registry = InteractorRegistry::new();
        let unknown = ComponentId::custom("unknown");
        assert!(
            registry.accepts_char_input(&unknown),
            "Unregistered components should default to accepting char input"
        );
    }

    #[test]
    fn test_registry_accepts_char_input_registered_accepting() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&ComponentId::EXPLORER));
    }

    #[test]
    fn test_registry_accepts_char_input_registered_keymap() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
    }

    #[test]
    fn test_registry_accepts_char_input_all_builtin_constants() {
        let registry = InteractorRegistry::new();
        // All built-in constants are unregistered in empty registry; default is true
        assert!(registry.accepts_char_input(&ComponentId::EDITOR));
        assert!(registry.accepts_char_input(&ComponentId::WINDOW));
        assert!(registry.accepts_char_input(&ComponentId::COMMAND_LINE));
        assert!(registry.accepts_char_input(&ComponentId::EXPLORER));
        assert!(registry.accepts_char_input(&ComponentId::TELESCOPE));
    }

    // ========================================================================
    // InteractorRegistry: uses_keymap
    // ========================================================================

    #[test]
    fn test_registry_uses_keymap_unregistered_defaults_false() {
        let registry = InteractorRegistry::new();
        let unknown = ComponentId::custom("unknown");
        assert!(
            !registry.uses_keymap(&unknown),
            "Unregistered components should default to NOT using keymap"
        );
    }

    #[test]
    fn test_registry_uses_keymap_registered_keymap() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        assert!(registry.uses_keymap(&ComponentId::WINDOW));
    }

    #[test]
    fn test_registry_uses_keymap_registered_accepting() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        assert!(!registry.uses_keymap(&ComponentId::EXPLORER));
    }

    #[test]
    fn test_registry_uses_keymap_is_inverse_of_accepts_char_input() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());

        // For registered components
        assert_ne!(
            registry.accepts_char_input(&ComponentId::WINDOW),
            registry.uses_keymap(&ComponentId::WINDOW),
            "uses_keymap should be inverse of accepts_char_input"
        );
        assert_ne!(
            registry.accepts_char_input(&ComponentId::EXPLORER),
            registry.uses_keymap(&ComponentId::EXPLORER),
            "uses_keymap should be inverse of accepts_char_input"
        );

        // For unregistered components
        let unknown = ComponentId::custom("unknown");
        assert_ne!(
            registry.accepts_char_input(&unknown),
            registry.uses_keymap(&unknown),
            "uses_keymap should be inverse of accepts_char_input for unregistered"
        );
    }

    // ========================================================================
    // InteractorRegistry: len and is_empty
    // ========================================================================

    #[test]
    fn test_registry_len_empty() {
        let registry = InteractorRegistry::new();
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_len_after_register() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        assert_eq!(registry.len(), 1);

        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_registry_len_after_unregister() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        assert_eq!(registry.len(), 2);

        registry.unregister(&ComponentId::WINDOW);
        assert_eq!(registry.len(), 1);

        registry.unregister(&ComponentId::EXPLORER);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_is_empty_true() {
        let registry = InteractorRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_is_empty_false() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_is_empty_after_all_removed() {
        let mut registry = InteractorRegistry::new();
        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        registry.unregister(&ComponentId::WINDOW);
        assert!(registry.is_empty());
    }

    // ========================================================================
    // InteractorRegistry: Debug
    // ========================================================================

    #[test]
    fn test_registry_debug_empty() {
        let registry = InteractorRegistry::new();
        let debug = format!("{registry:?}");
        assert!(
            debug.contains("InteractorRegistry"),
            "Debug output should contain the type name"
        );
    }

    #[test]
    fn test_registry_debug_with_entries() {
        let registry = InteractorRegistry::with_builtins();
        let debug = format!("{registry:?}");
        assert!(debug.contains("InteractorRegistry"));
    }

    // ========================================================================
    // Integration scenarios
    // ========================================================================

    #[test]
    fn test_typical_setup_scenario() {
        // Simulates typical module initialization:
        // 1. Create registry with builtins
        // 2. Add custom interactors from plugins
        // 3. Query behavior at runtime
        let mut registry = InteractorRegistry::with_builtins();

        // Window mode uses keymap (from builtins)
        assert!(registry.uses_keymap(&ComponentId::WINDOW));

        // Explorer plugin registers as accepting input
        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&ComponentId::EXPLORER));

        // Telescope plugin registers as accepting input
        registry.register(ComponentId::TELESCOPE, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&ComponentId::TELESCOPE));

        // Custom plugin registers with keymap
        let panel = ComponentId::custom("file_tree");
        registry.register(panel, InteractorConfig::using_keymap());
        assert!(registry.uses_keymap(&panel));

        // Editor is unregistered, defaults to accepting input
        assert!(registry.accepts_char_input(&ComponentId::EDITOR));

        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn test_mode_transition_scenario() {
        // Simulates a component switching between input modes
        let mut registry = InteractorRegistry::new();

        let explorer = ComponentId::EXPLORER;

        // Explorer initially in navigation mode (keymap)
        registry.register(explorer, InteractorConfig::using_keymap());
        assert!(registry.uses_keymap(&explorer));

        // Explorer switches to filter mode (text input)
        registry.register(explorer, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&explorer));

        // Explorer switches back to navigation mode
        registry.register(explorer, InteractorConfig::using_keymap());
        assert!(registry.uses_keymap(&explorer));
    }

    #[test]
    fn test_plugin_lifecycle_scenario() {
        // Simulates plugin load/unload cycle
        let mut registry = InteractorRegistry::with_builtins();

        let plugin_id = ComponentId::custom("my_plugin");

        // Plugin loads and registers
        registry.register(plugin_id, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&plugin_id));

        // Plugin unloads
        registry.unregister(&plugin_id);
        assert!(registry.get(&plugin_id).is_none());

        // After unload, defaults to accepting (safe default)
        assert!(registry.accepts_char_input(&plugin_id));
    }

    #[test]
    fn test_many_custom_components() {
        let mut registry = InteractorRegistry::new();

        for i in 0..100 {
            // Use leaked string to get 'static lifetime for testing
            let name: &'static str = Box::leak(format!("component_{i}").into_boxed_str());
            let id = ComponentId::new(name);
            if i % 2 == 0 {
                registry.register(id, InteractorConfig::accepting_input());
            } else {
                registry.register(id, InteractorConfig::using_keymap());
            }
        }

        assert_eq!(registry.len(), 100);
    }
}
