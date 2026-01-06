//! Interactor configuration and registry
//!
//! This module provides metadata-driven configuration for interactor components,
//! allowing them to declare their input handling behavior instead of relying on
//! hardcoded checks in the mode system.

use std::collections::HashMap;

use crate::modd::ComponentId;

/// Configuration for an interactor component
///
/// Defines how an interactor handles input. Components that accept character input
/// (like Explorer's filter mode) will receive printable characters directly.
/// Components that use keymap (like Window mode) will have characters looked up
/// in the keymap instead.
#[derive(Debug, Clone, Copy)]
pub struct InteractorConfig {
    /// Whether this interactor accepts character input (insert-mode style)
    ///
    /// When `true`, single printable characters are routed as text input via
    /// `PluginTextInput` events. When `false`, characters are looked up in the
    /// keymap and dispatched as commands.
    pub accepts_char_input: bool,
}

impl InteractorConfig {
    /// Create config that accepts character input (default for most plugins)
    ///
    /// Use this for interactors that need text input, like:
    /// - Explorer's create/rename/filter modes
    /// - Telescope's search input
    /// - Leap/jump search modes
    #[must_use]
    pub const fn accepting_input() -> Self {
        Self {
            accepts_char_input: true,
        }
    }

    /// Create config that uses keymap (like Window mode)
    ///
    /// Use this for interactors where keys trigger commands:
    /// - Window mode (h/j/k/l for navigation)
    /// - Any mode where characters map to actions
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

/// Registry for interactor configurations
///
/// Stores metadata for interactor components, allowing the mode system to
/// query behavior without hardcoding component-specific logic.
#[derive(Debug, Default)]
pub struct InteractorRegistry {
    configs: HashMap<ComponentId, InteractorConfig>,
}

impl InteractorRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register configuration for an interactor
    ///
    /// If the interactor was already registered, the config is replaced.
    pub fn register(&mut self, id: ComponentId, config: InteractorConfig) {
        self.configs.insert(id, config);
    }

    /// Get configuration for an interactor
    ///
    /// Returns `None` if the interactor is not registered.
    #[must_use]
    pub fn get(&self, id: &ComponentId) -> Option<&InteractorConfig> {
        self.configs.get(id)
    }

    /// Check if interactor accepts character input
    ///
    /// Returns `true` by default for unregistered interactors (safe default
    /// that matches the behavior of most plugins).
    #[must_use]
    pub fn accepts_char_input(&self, id: &ComponentId) -> bool {
        self.get(id).is_none_or(|config| config.accepts_char_input)
    }

    /// Register all built-in interactor configs
    ///
    /// This should be called during runtime initialization to set up
    /// core component configurations.
    pub fn register_builtins(&mut self) {
        // Window mode does NOT accept char input - uses keymap for h/j/k/l
        self.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_registry_accepts_char_input() {
        let mut registry = InteractorRegistry::new();

        // Unregistered interactor defaults to accepting input
        let unknown = ComponentId("unknown");
        assert!(registry.accepts_char_input(&unknown));

        // Register WINDOW as not accepting input
        registry.register_builtins();
        assert!(!registry.accepts_char_input(&ComponentId::WINDOW));

        // Register custom interactor
        let explorer = ComponentId("explorer");
        registry.register(explorer, InteractorConfig::accepting_input());
        assert!(registry.accepts_char_input(&explorer));
    }

    #[test]
    fn test_registry_get() {
        let mut registry = InteractorRegistry::new();
        registry.register_builtins();

        assert!(registry.get(&ComponentId::WINDOW).is_some());
        assert!(registry.get(&ComponentId("unknown")).is_none());
    }
}
