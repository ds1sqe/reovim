//! Extensible focus system for keyboard-focusable UI components
//!
//! The focus system uses a trait-based approach allowing plugins to register
//! custom focus targets without modifying core code.

use std::collections::HashMap;

/// Unique identifier for a focus target
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FocusId(pub &'static str);

impl FocusId {
    /// Editor focus (main text editing area)
    pub const EDITOR: Self = Self("editor");
    /// Explorer focus (file browser sidebar)
    pub const EXPLORER: Self = Self("explorer");
    /// Telescope focus (fuzzy finder)
    pub const TELESCOPE: Self = Self("telescope");
    /// Settings menu focus
    pub const SETTINGS: Self = Self("settings");
}

impl std::fmt::Display for FocusId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for FocusId {
    fn default() -> Self {
        Self::EDITOR
    }
}

/// Trait for components that can receive keyboard focus
///
/// Implement this trait to create custom focus targets that can be registered
/// with the [`FocusRegistry`].
pub trait FocusTarget: std::fmt::Debug + Send + Sync {
    /// Unique identifier for this focus target
    fn id(&self) -> FocusId;

    /// Display name for status line
    fn display_name(&self) -> &'static str;

    /// Handle character input when in insert mode (if supported)
    ///
    /// Returns `true` if the character was handled, `false` otherwise.
    fn insert_char(&mut self, _c: char) -> bool {
        false
    }

    /// Handle backspace in insert mode (if supported)
    ///
    /// Returns `true` if backspace was handled, `false` otherwise.
    fn delete_char_backward(&mut self) -> bool {
        false
    }

    /// Icon for status line (optional)
    fn icon(&self) -> Option<&'static str> {
        None
    }
}

/// Registry of focus targets
///
/// Manages all registered focus targets and tracks the currently active one.
#[derive(Debug)]
pub struct FocusRegistry {
    targets: HashMap<FocusId, Box<dyn FocusTarget>>,
    active: FocusId,
}

impl Default for FocusRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusRegistry {
    /// Create a new empty focus registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            targets: HashMap::new(),
            active: FocusId::EDITOR,
        }
    }

    /// Register a focus target
    ///
    /// If a target with the same ID already exists, it will be replaced.
    pub fn register(&mut self, target: Box<dyn FocusTarget>) {
        let id = target.id();
        self.targets.insert(id, target);
    }

    /// Set the active focus target
    ///
    /// Returns `true` if the focus was changed, `false` if the target ID is not registered.
    pub fn set_active(&mut self, id: FocusId) -> bool {
        if self.targets.contains_key(&id) {
            self.active = id;
            true
        } else {
            false
        }
    }

    /// Get the active focus target
    ///
    /// # Panics
    ///
    /// Panics if the active focus target is not registered (should never happen in normal use).
    #[must_use]
    pub fn active(&self) -> &dyn FocusTarget {
        self.targets
            .get(&self.active)
            .map(AsRef::as_ref)
            .expect("active focus target should always be registered")
    }

    /// Get the active focus target mutably
    ///
    /// # Panics
    ///
    /// Panics if the active focus target is not registered.
    pub fn active_mut(&mut self) -> &mut Box<dyn FocusTarget> {
        self.targets
            .get_mut(&self.active)
            .expect("active focus target should always be registered")
    }

    /// Get the active focus ID
    #[must_use]
    pub const fn active_id(&self) -> FocusId {
        self.active
    }

    /// Get a focus target by ID
    #[must_use]
    pub fn get(&self, id: FocusId) -> Option<&dyn FocusTarget> {
        self.targets.get(&id).map(AsRef::as_ref)
    }

    /// Get a focus target by ID mutably
    pub fn get_mut(&mut self, id: FocusId) -> Option<&mut Box<dyn FocusTarget>> {
        self.targets.get_mut(&id)
    }

    /// Check if a focus target is registered
    #[must_use]
    pub fn contains(&self, id: FocusId) -> bool {
        self.targets.contains_key(&id)
    }

    /// Get the number of registered focus targets
    #[must_use]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Create a registry with default focus targets registered
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(EditorFocus));
        registry.register(Box::new(ExplorerFocus::default()));
        registry.register(Box::new(TelescopeFocus::default()));
        registry.register(Box::new(SettingsFocus));
        registry
    }
}

// Built-in focus targets

/// Editor focus target (main text editing area)
#[derive(Debug, Default)]
pub struct EditorFocus;

impl FocusTarget for EditorFocus {
    fn id(&self) -> FocusId {
        FocusId::EDITOR
    }

    fn display_name(&self) -> &'static str {
        "EDITOR"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }
}

/// Explorer focus target (file browser sidebar)
///
/// Handles filename input for create/rename operations.
#[derive(Debug, Default)]
pub struct ExplorerFocus {
    /// Current input buffer for filename operations
    pub input_buffer: String,
    /// Whether input mode is active
    pub input_active: bool,
}

impl FocusTarget for ExplorerFocus {
    fn id(&self) -> FocusId {
        FocusId::EXPLORER
    }

    fn display_name(&self) -> &'static str {
        "EXPLORER"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn insert_char(&mut self, c: char) -> bool {
        if self.input_active {
            self.input_buffer.push(c);
            true
        } else {
            false
        }
    }

    fn delete_char_backward(&mut self) -> bool {
        if self.input_active {
            self.input_buffer.pop();
            true
        } else {
            false
        }
    }
}

/// Telescope focus target (fuzzy finder)
///
/// Handles query input for fuzzy searching.
#[derive(Debug, Default)]
pub struct TelescopeFocus {
    /// Current query string
    pub query: String,
}

impl FocusTarget for TelescopeFocus {
    fn id(&self) -> FocusId {
        FocusId::TELESCOPE
    }

    fn display_name(&self) -> &'static str {
        "TELESCOPE"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn insert_char(&mut self, c: char) -> bool {
        self.query.push(c);
        true
    }

    fn delete_char_backward(&mut self) -> bool {
        self.query.pop();
        true
    }
}

/// Settings menu focus target
#[derive(Debug, Default)]
pub struct SettingsFocus;

impl FocusTarget for SettingsFocus {
    fn id(&self) -> FocusId {
        FocusId::SETTINGS
    }

    fn display_name(&self) -> &'static str {
        "SETTINGS"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_id_equality() {
        assert_eq!(FocusId::EDITOR, FocusId("editor"));
        assert_ne!(FocusId::EDITOR, FocusId::EXPLORER);
    }

    #[test]
    fn test_focus_registry_register() {
        let mut registry = FocusRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(EditorFocus));
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(FocusId::EDITOR));
    }

    #[test]
    fn test_focus_registry_active() {
        let mut registry = FocusRegistry::new();
        registry.register(Box::new(EditorFocus));
        registry.register(Box::new(TelescopeFocus::default()));

        assert_eq!(registry.active_id(), FocusId::EDITOR);

        assert!(registry.set_active(FocusId::TELESCOPE));
        assert_eq!(registry.active_id(), FocusId::TELESCOPE);

        // Can't set to unregistered target
        assert!(!registry.set_active(FocusId::EXPLORER));
        assert_eq!(registry.active_id(), FocusId::TELESCOPE);
    }

    #[test]
    fn test_telescope_focus_insert_char() {
        let mut focus = TelescopeFocus::default();
        assert!(focus.insert_char('a'));
        assert!(focus.insert_char('b'));
        assert_eq!(focus.query, "ab");

        assert!(focus.delete_char_backward());
        assert_eq!(focus.query, "a");
    }

    #[test]
    fn test_explorer_focus_input() {
        let mut focus = ExplorerFocus::default();

        // Not active, should not insert
        assert!(!focus.insert_char('a'));
        assert!(focus.input_buffer.is_empty());

        // Activate input
        focus.input_active = true;
        assert!(focus.insert_char('t'));
        assert!(focus.insert_char('e'));
        assert!(focus.insert_char('s'));
        assert!(focus.insert_char('t'));
        assert_eq!(focus.input_buffer, "test");
    }
}
