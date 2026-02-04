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

    #[test]
    fn test_component_id_constants() {
        assert_eq!(ComponentId::EDITOR.as_str(), "editor");
        assert_eq!(ComponentId::WINDOW.as_str(), "window");
        assert_eq!(ComponentId::COMMAND_LINE.as_str(), "command_line");
    }

    #[test]
    fn test_component_id_equality() {
        assert_eq!(ComponentId::EDITOR, ComponentId::new("editor"));
        assert_ne!(ComponentId::EDITOR, ComponentId::WINDOW);
    }

    #[test]
    fn test_component_id_custom() {
        let custom = ComponentId::custom("my_plugin");
        assert_eq!(custom.as_str(), "my_plugin");
    }

    #[test]
    fn test_interactor_config_defaults() {
        let default = InteractorConfig::default();
        assert!(default.accepts_char_input);

        let accepting = InteractorConfig::accepting_input();
        assert!(accepting.accepts_char_input);

        let keymap = InteractorConfig::using_keymap();
        assert!(!keymap.accepts_char_input);
    }

    #[test]
    fn test_registry_new() {
        let registry = InteractorRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_with_builtins() {
        let registry = InteractorRegistry::with_builtins();
        assert!(!registry.is_empty());
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
    }

    #[test]
    fn test_registry_accepts_char_input() {
        let mut registry = InteractorRegistry::new();

        // Unregistered interactor defaults to accepting input
        let unknown = ComponentId::custom("unknown");
        assert!(registry.accepts_char_input(&unknown));

        // Register WINDOW as not accepting input
        registry.register_builtins();
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));

        // Register custom interactor
        let explorer = ComponentId::EXPLORER;
        registry.register(explorer, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&explorer));
    }

    #[test]
    fn test_registry_uses_keymap() {
        let mut registry = InteractorRegistry::new();
        registry.register_builtins();

        assert!(registry.uses_keymap(&ComponentId::WINDOW));
        assert!(!registry.uses_keymap(&ComponentId::EDITOR));
    }

    #[test]
    fn test_registry_get() {
        let mut registry = InteractorRegistry::new();
        registry.register_builtins();

        assert!(registry.get(&ComponentId::WINDOW).is_some());
        assert!(registry.get(&ComponentId::custom("unknown")).is_none());
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = InteractorRegistry::with_builtins();
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));

        registry.unregister(&ComponentId::WINDOW);
        // After unregistration, defaults to accepting input
        assert!(registry.accepts_char_input(&ComponentId::WINDOW));
    }

    #[test]
    fn test_registry_len() {
        let mut registry = InteractorRegistry::new();
        assert_eq!(registry.len(), 0);

        registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
        assert_eq!(registry.len(), 1);

        registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
        assert_eq!(registry.len(), 2);
    }
}
