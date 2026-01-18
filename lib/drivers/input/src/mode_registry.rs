//! Mode registry for O(1) behavior queries.
//!
//! This module provides `ModeRegistry`, which stores mode behavior information
//! and provides O(1) queries for common mode properties.
//!
//! # Architecture
//!
//! Policy modules (e.g., vim) register their modes at initialization.
//! The registry caches behavior flags in `HashSet`s for O(1) lookup:
//!
//! ```text
//! VimMode::ALL.iter() ──register()──> ModeRegistry
//!                                         │
//!                                         ├── entries: HashMap<ModeId, ModeEntry>
//!                                         ├── input_modes: HashSet<ModeId>
//!                                         └── selection_modes: HashSet<ModeId>
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_input::ModeRegistry;
//! use my_vim_module::VimMode;
//!
//! let mut registry = ModeRegistry::new();
//!
//! // Register all vim modes
//! registry.register_all(VimMode::ALL.iter().copied());
//!
//! // O(1) queries
//! assert!(registry.accepts_char_input(&VimMode::Insert.id()));
//! assert!(registry.has_selection(&VimMode::Visual.id()));
//! ```

use std::collections::{HashMap, HashSet};

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

/// Entry in the mode registry.
///
/// Stores all behavior information for a single mode.
#[derive(Debug, Clone)]
pub struct ModeEntry {
    /// Display name for statusline.
    pub display_name: &'static str,
    /// Cursor style for this mode.
    pub cursor_style: CursorStyle,
    /// Whether this mode accepts character input.
    pub accepts_char_input: bool,
    /// Whether this mode has an active selection.
    pub has_selection: bool,
    /// Parent mode for keybinding inheritance.
    pub inherits_from: Option<ModeId>,
}

/// Registry for looking up mode behavior by `ModeId`.
///
/// Provides O(1) queries for common mode properties using cached `HashSet`s.
#[derive(Debug, Default)]
pub struct ModeRegistry {
    /// All registered modes.
    entries: HashMap<ModeId, ModeEntry>,

    /// Cached set of modes that accept character input.
    input_modes: HashSet<ModeId>,

    /// Cached set of modes that have an active selection.
    selection_modes: HashSet<ModeId>,
}

impl ModeRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a mode.
    ///
    /// Called by policy modules at initialization to register all their modes.
    /// Updates all caches immediately.
    pub fn register<M: Mode>(&mut self, mode: M) {
        let id = mode.id();
        let entry = ModeEntry {
            display_name: mode.display_name(),
            cursor_style: mode.cursor_style(),
            accepts_char_input: mode.accepts_char_input(),
            has_selection: mode.has_selection(),
            inherits_from: mode.inherits_from().map(|m| m.id()),
        };

        // Update caches immediately
        if entry.accepts_char_input {
            self.input_modes.insert(id.clone());
        }
        if entry.has_selection {
            self.selection_modes.insert(id.clone());
        }

        self.entries.insert(id, entry);
    }

    /// Register all variants of a Mode enum.
    ///
    /// Convenience method for registering all modes at once.
    pub fn register_all<M: Mode, I: IntoIterator<Item = M>>(&mut self, modes: I) {
        for mode in modes {
            self.register(mode);
        }
    }

    // ========================================================================
    // O(1) Queries
    // ========================================================================

    /// Check if a mode accepts character input.
    ///
    /// Returns `true` for modes like Insert, `CommandLine`, Replace.
    /// Returns `false` for unknown modes.
    #[must_use]
    pub fn accepts_char_input(&self, id: &ModeId) -> bool {
        self.input_modes.contains(id)
    }

    /// Check if a mode has an active selection.
    ///
    /// Returns `true` for Visual, Select modes.
    /// Returns `false` for unknown modes.
    #[must_use]
    pub fn has_selection(&self, id: &ModeId) -> bool {
        self.selection_modes.contains(id)
    }

    /// Get the display name for a mode.
    ///
    /// Returns "UNKNOWN" for unregistered modes.
    #[must_use]
    pub fn display_name(&self, id: &ModeId) -> &'static str {
        self.entries.get(id).map_or("UNKNOWN", |e| e.display_name)
    }

    /// Get the cursor style for a mode.
    ///
    /// Returns `CursorStyle::Block` for unregistered modes.
    #[must_use]
    pub fn cursor_style(&self, id: &ModeId) -> CursorStyle {
        self.entries
            .get(id)
            .map_or(CursorStyle::Block, |e| e.cursor_style)
    }

    /// Get the parent mode for keybinding inheritance.
    ///
    /// Returns `None` for unregistered modes or root modes.
    #[must_use]
    pub fn inherits_from(&self, id: &ModeId) -> Option<&ModeId> {
        self.entries.get(id).and_then(|e| e.inherits_from.as_ref())
    }

    /// Get the full entry for a mode.
    ///
    /// Returns `None` for unregistered modes.
    #[must_use]
    pub fn get(&self, id: &ModeId) -> Option<&ModeEntry> {
        self.entries.get(id)
    }

    /// Check if a mode is registered.
    #[must_use]
    pub fn contains(&self, id: &ModeId) -> bool {
        self.entries.contains_key(id)
    }

    /// Get the number of registered modes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Unregister all modes from a module.
    ///
    /// Returns the number of modes removed.
    pub fn unregister_module(&mut self, module: &ModuleId) -> usize {
        let to_remove: Vec<_> = self
            .entries
            .keys()
            .filter(|id| id.module() == module)
            .cloned()
            .collect();

        for id in &to_remove {
            self.entries.remove(id);
            self.input_modes.remove(id);
            self.selection_modes.remove(id);
        }

        to_remove.len()
    }

    /// Get an iterator over all registered mode IDs.
    pub fn mode_ids(&self) -> impl Iterator<Item = &ModeId> {
        self.entries.keys()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test Mode implementation
    const TEST_MODULE: ModuleId = ModuleId::new("test");

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u16)]
    enum TestMode {
        Normal = 0,
        Insert = 1,
        Visual = 2,
    }

    impl Mode for TestMode {
        fn module() -> ModuleId {
            TEST_MODULE
        }

        fn discriminant(&self) -> u16 {
            *self as u16
        }

        fn display_name(&self) -> &'static str {
            match self {
                Self::Normal => "NORMAL",
                Self::Insert => "INSERT",
                Self::Visual => "VISUAL",
            }
        }

        fn cursor_style(&self) -> CursorStyle {
            match self {
                Self::Insert => CursorStyle::Bar,
                _ => CursorStyle::Block,
            }
        }

        fn accepts_char_input(&self) -> bool {
            matches!(self, Self::Insert)
        }

        fn has_selection(&self) -> bool {
            matches!(self, Self::Visual)
        }

        fn inherits_from(&self) -> Option<Self> {
            match self {
                Self::Visual => Some(Self::Normal),
                _ => None,
            }
        }
    }

    impl TestMode {
        const ALL: &'static [Self] = &[Self::Normal, Self::Insert, Self::Visual];
    }

    #[test]
    fn test_registry_new() {
        let registry = ModeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ModeRegistry::new();
        registry.register(TestMode::Normal);

        assert_eq!(registry.len(), 1);
        assert!(registry.contains(&TestMode::Normal.id()));
    }

    #[test]
    fn test_registry_register_all() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        assert_eq!(registry.len(), 3);
        assert!(registry.contains(&TestMode::Normal.id()));
        assert!(registry.contains(&TestMode::Insert.id()));
        assert!(registry.contains(&TestMode::Visual.id()));
    }

    #[test]
    fn test_registry_accepts_char_input() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        assert!(!registry.accepts_char_input(&TestMode::Normal.id()));
        assert!(registry.accepts_char_input(&TestMode::Insert.id()));
        assert!(!registry.accepts_char_input(&TestMode::Visual.id()));
    }

    #[test]
    fn test_registry_has_selection() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        assert!(!registry.has_selection(&TestMode::Normal.id()));
        assert!(!registry.has_selection(&TestMode::Insert.id()));
        assert!(registry.has_selection(&TestMode::Visual.id()));
    }

    #[test]
    fn test_registry_display_name() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        assert_eq!(registry.display_name(&TestMode::Normal.id()), "NORMAL");
        assert_eq!(registry.display_name(&TestMode::Insert.id()), "INSERT");
        assert_eq!(registry.display_name(&TestMode::Visual.id()), "VISUAL");
    }

    #[test]
    fn test_registry_cursor_style() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        assert_eq!(registry.cursor_style(&TestMode::Normal.id()), CursorStyle::Block);
        assert_eq!(registry.cursor_style(&TestMode::Insert.id()), CursorStyle::Bar);
        assert_eq!(registry.cursor_style(&TestMode::Visual.id()), CursorStyle::Block);
    }

    #[test]
    fn test_registry_inherits_from() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        assert_eq!(registry.inherits_from(&TestMode::Normal.id()), None);
        assert_eq!(registry.inherits_from(&TestMode::Insert.id()), None);
        assert_eq!(registry.inherits_from(&TestMode::Visual.id()), Some(&TestMode::Normal.id()));
    }

    #[test]
    fn test_registry_unknown_mode() {
        let registry = ModeRegistry::new();
        let unknown = ModeId::with_discriminant(ModuleId::new("unknown"), "TEST", 99);

        // Should return defaults without panicking
        assert!(!registry.accepts_char_input(&unknown));
        assert!(!registry.has_selection(&unknown));
        assert_eq!(registry.display_name(&unknown), "UNKNOWN");
        assert_eq!(registry.cursor_style(&unknown), CursorStyle::Block);
        assert_eq!(registry.inherits_from(&unknown), None);
    }

    #[test]
    fn test_registry_unregister_module() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        assert_eq!(registry.len(), 3);
        assert!(registry.accepts_char_input(&TestMode::Insert.id()));

        // Unregister all test modes
        let removed = registry.unregister_module(&TEST_MODULE);

        assert_eq!(removed, 3);
        assert!(registry.is_empty());
        assert!(!registry.accepts_char_input(&TestMode::Insert.id()));
        assert!(!registry.has_selection(&TestMode::Visual.id()));
    }

    #[test]
    fn test_registry_cache_coherence_on_register() {
        let mut registry = ModeRegistry::new();

        // Register Insert mode (accepts char input)
        registry.register(TestMode::Insert);

        // Verify immediately queryable
        assert!(registry.accepts_char_input(&TestMode::Insert.id()));
        assert_eq!(registry.display_name(&TestMode::Insert.id()), "INSERT");
    }

    #[test]
    fn test_registry_cache_coherence_on_unregister() {
        let mut registry = ModeRegistry::new();
        registry.register_all(TestMode::ALL.iter().copied());

        // Verify registered
        assert!(registry.accepts_char_input(&TestMode::Insert.id()));
        assert!(registry.has_selection(&TestMode::Visual.id()));

        // Unregister module
        registry.unregister_module(&TEST_MODULE);

        // Verify cleared from ALL caches
        assert!(!registry.accepts_char_input(&TestMode::Insert.id()));
        assert!(!registry.has_selection(&TestMode::Visual.id()));
        assert_eq!(registry.display_name(&TestMode::Insert.id()), "UNKNOWN");
    }
}
