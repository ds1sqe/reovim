//! Mode registry for storing mode metadata and behavior.
//!
//! This registry stores mode behavior information directly, keyed by `ModeId`.
//! The `Mode` trait is NOT object-safe (by design), so we store mode properties
//! directly rather than trait objects.
//!
//! # Architecture (Epic #372)
//!
//! - `Mode` trait: Compile-time type-safe mode definitions (not object-safe)
//! - `ModeId`: Runtime identity stored in `ModeStack`, used as registry keys
//! - `ModeEntry`: Cached behavior properties (cursor style, input acceptance)
//!
//! # Registration
//!
//! Modes are registered via [`ModeRegistry::register`] or convenience methods
//! like [`ModeRegistry::register_mode`] and [`ModeRegistry::register_all`].

use std::collections::HashMap;

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

/// Entry in the mode registry containing cached mode behavior.
///
/// Stores mode identity and behavior properties directly. The `Mode` trait
/// is not object-safe, so we cache the properties here for runtime lookup.
#[derive(Debug, Clone)]
pub struct ModeEntry {
    /// The mode ID.
    id: ModeId,

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

    /// Whether this is the entry/default mode for new sessions.
    pub is_entry: bool,

    /// The module that owns this mode (if any).
    ///
    /// `None` for built-in modes or modes registered without ownership.
    owner: Option<ModuleId>,
}

impl ModeEntry {
    /// Create a new mode entry from a Mode implementation.
    ///
    /// Caches all behavior properties from the Mode trait.
    #[must_use]
    pub fn from_mode<M: Mode>(mode: M) -> Self {
        Self {
            id: mode.id(),
            display_name: mode.display_name(),
            cursor_style: mode.cursor_style(),
            accepts_char_input: mode.accepts_char_input(),
            has_selection: mode.has_selection(),
            inherits_from: mode.inherits_from().map(|m| m.id()),
            is_entry: mode.is_entry(),
            owner: None,
        }
    }

    /// Create a mode entry from `ModeInfo` (Epic #417 Part 3).
    ///
    /// Used when extracting modes from `ModeInfoStore`.
    #[must_use]
    pub fn from_info(info: reovim_driver_input::ModeInfo) -> Self {
        Self {
            id: info.id,
            display_name: info.display_name,
            cursor_style: info.cursor_style,
            accepts_char_input: info.accepts_char_input,
            has_selection: info.has_selection,
            inherits_from: info.inherits_from,
            is_entry: info.is_entry,
            owner: None,
        }
    }

    /// Set the owning module for this mode entry.
    #[must_use]
    pub fn with_owner(mut self, owner: ModuleId) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Get the mode's ID.
    #[must_use]
    pub const fn id(&self) -> &ModeId {
        &self.id
    }

    /// Get the owning module ID if any.
    #[must_use]
    pub const fn owner(&self) -> Option<&ModuleId> {
        self.owner.as_ref()
    }
}

/// Registry for mode metadata and behavior.
///
/// Stores [`ModeEntry`] instances keyed by [`ModeId`]. The server
/// uses this to look up cursor styles and input behavior for the
/// current mode.
///
/// # Entry Mode Auto-Detection
///
/// When modes are registered via [`register`](Self::register) or
/// [`register_mode`](Self::register_mode), the registry automatically
/// detects which mode is the entry mode (first mode with `is_entry = true`).
/// This is used by session creation to determine the initial mode.
#[derive(Default, Debug)]
pub struct ModeRegistry {
    modes: HashMap<ModeId, ModeEntry>,
    /// The auto-detected entry mode (first mode registered with `is_entry` = true).
    entry_mode: Option<ModeId>,
}

impl ModeRegistry {
    /// Create a new empty mode registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a mode from a Mode implementation.
    ///
    /// Convenience method that creates a `ModeEntry` from the mode.
    pub fn register_mode<M: Mode>(&mut self, mode: M) {
        self.register(ModeEntry::from_mode(mode));
    }

    /// Register all modes from an iterator.
    ///
    /// Convenience method for registering multiple modes at once.
    pub fn register_all<M: Mode, I: IntoIterator<Item = M>>(&mut self, modes: I) {
        for mode in modes {
            self.register_mode(mode);
        }
    }

    /// Register a mode entry.
    ///
    /// If a mode with the same ID already exists, it is replaced.
    /// Auto-detects entry mode: the first mode registered with `is_entry = true`
    /// becomes the session's initial mode.
    pub fn register(&mut self, entry: ModeEntry) {
        // Auto-detect entry mode (first one wins)
        if entry.is_entry && self.entry_mode.is_none() {
            self.entry_mode = Some(entry.id.clone());
        }
        let id = entry.id.clone();
        self.modes.insert(id, entry);
    }

    /// Get the auto-detected entry mode.
    ///
    /// Returns the first mode that was registered with `is_entry = true`,
    /// or `None` if no entry mode was found.
    #[must_use]
    pub const fn entry_mode(&self) -> Option<&ModeId> {
        self.entry_mode.as_ref()
    }

    /// Get a mode entry by ID.
    #[must_use]
    pub fn get(&self, id: &ModeId) -> Option<&ModeEntry> {
        self.modes.get(id)
    }

    /// Check if a mode is registered.
    #[must_use]
    pub fn contains(&self, id: &ModeId) -> bool {
        self.modes.contains_key(id)
    }

    /// Check if a mode accepts character input.
    ///
    /// Returns `false` if the mode isn't registered.
    #[must_use]
    pub fn accepts_char_input(&self, id: &ModeId) -> bool {
        self.modes.get(id).is_some_and(|e| e.accepts_char_input)
    }

    /// Check if a mode has an active selection.
    ///
    /// Returns `false` if the mode isn't registered.
    #[must_use]
    pub fn has_selection(&self, id: &ModeId) -> bool {
        self.modes.get(id).is_some_and(|e| e.has_selection)
    }

    /// Get the cursor style for a mode.
    ///
    /// Returns `CursorStyle::Block` (default) if the mode isn't registered.
    #[must_use]
    pub fn cursor_style(&self, id: &ModeId) -> CursorStyle {
        self.modes
            .get(id)
            .map_or(CursorStyle::Block, |e| e.cursor_style)
    }

    /// Get the display name for a mode.
    ///
    /// Returns "UNKNOWN" if the mode isn't registered.
    #[must_use]
    pub fn display_name(&self, id: &ModeId) -> &'static str {
        self.modes.get(id).map_or("UNKNOWN", |e| e.display_name)
    }

    /// Get the parent mode for keybinding inheritance.
    ///
    /// Returns `None` if the mode isn't registered or has no parent.
    #[must_use]
    pub fn inherits_from(&self, id: &ModeId) -> Option<&ModeId> {
        self.modes.get(id).and_then(|e| e.inherits_from.as_ref())
    }

    /// Get all registered mode IDs.
    pub fn ids(&self) -> impl Iterator<Item = &ModeId> {
        self.modes.keys()
    }

    /// Find a mode by module and name strings.
    ///
    /// This is used during keybinding wiring to look up the correct `ModeId`
    /// (with proper discriminant) from a mode name like "editor:normal".
    ///
    /// Returns `None` if no mode with the given module and name is registered.
    #[must_use]
    pub fn find_by_name(&self, module: &str, name: &str) -> Option<&ModeId> {
        // Linear search - acceptable because mode count is small (<20)
        // Compare against entry.id.name() (the programmatic name like "visual-block")
        // not display_name (the statusline display like "V-BLOCK")
        self.modes.values().find_map(|entry| {
            if entry.id.module().as_str() == module && entry.id.name() == name {
                Some(&entry.id)
            } else {
                None
            }
        })
    }

    /// Get the number of registered modes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modes.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }

    /// Remove all modes owned by a module.
    ///
    /// Called when a module is being unloaded to clean up its registrations.
    ///
    /// Returns the number of modes that were removed.
    pub fn unregister_for_module(&mut self, module: &ModuleId) -> usize {
        let before = self.modes.len();
        self.modes
            .retain(|_, entry| entry.owner.as_ref() != Some(module));
        before - self.modes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test mode enum implementing the Mode trait
    const TEST_MODULE: ModuleId = ModuleId::new("test");

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u16)]
    enum TestMode {
        Command = 0,
        Input = 1,
        Selection = 2,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Mode for TestMode {
        fn module() -> ModuleId {
            TEST_MODULE
        }

        fn discriminant(&self) -> u16 {
            *self as u16
        }

        fn id(&self) -> ModeId {
            ModeId::with_discriminant(TEST_MODULE, self.display_name(), self.discriminant())
        }

        fn display_name(&self) -> &'static str {
            match self {
                Self::Command => "COMMAND",
                Self::Input => "INPUT",
                Self::Selection => "SELECTION",
            }
        }

        fn cursor_style(&self) -> CursorStyle {
            match self {
                Self::Command | Self::Selection => CursorStyle::Block,
                Self::Input => CursorStyle::Bar,
            }
        }

        fn accepts_char_input(&self) -> bool {
            matches!(self, Self::Input)
        }

        fn has_selection(&self) -> bool {
            matches!(self, Self::Selection)
        }

        fn inherits_from(&self) -> Option<Self> {
            match self {
                Self::Selection => Some(Self::Command),
                _ => None,
            }
        }

        fn is_entry(&self) -> bool {
            matches!(self, Self::Command)
        }
    }

    #[test]
    fn test_mode_registry_new() {
        let registry = ModeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_mode_registry_register() {
        let mut registry = ModeRegistry::new();
        registry.register_mode(TestMode::Command);

        assert!(registry.contains(&TestMode::Command.id()));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_mode_registry_register_all() {
        let mut registry = ModeRegistry::new();
        registry.register_all([TestMode::Command, TestMode::Input, TestMode::Selection]);

        assert_eq!(registry.len(), 3);
        assert!(registry.contains(&TestMode::Command.id()));
        assert!(registry.contains(&TestMode::Input.id()));
        assert!(registry.contains(&TestMode::Selection.id()));
    }

    #[test]
    fn test_mode_registry_accepts_char_input() {
        let mut registry = ModeRegistry::new();
        registry.register_all([TestMode::Command, TestMode::Input]);

        assert!(!registry.accepts_char_input(&TestMode::Command.id()));
        assert!(registry.accepts_char_input(&TestMode::Input.id()));
    }

    #[test]
    fn test_mode_registry_cursor_style() {
        let mut registry = ModeRegistry::new();
        registry.register_all([TestMode::Command, TestMode::Input]);

        assert_eq!(registry.cursor_style(&TestMode::Command.id()), CursorStyle::Block);
        assert_eq!(registry.cursor_style(&TestMode::Input.id()), CursorStyle::Bar);
    }

    #[test]
    fn test_mode_registry_entry_mode_auto_detect() {
        let mut registry = ModeRegistry::new();

        // Registry starts with no entry mode
        assert!(registry.entry_mode().is_none());

        // Register entry mode (Command)
        registry.register_mode(TestMode::Command);
        assert_eq!(registry.entry_mode(), Some(&TestMode::Command.id()));
    }

    #[test]
    fn test_mode_entry_with_owner() {
        let entry = ModeEntry::from_mode(TestMode::Command);
        assert!(entry.owner().is_none());

        let owned = entry.with_owner(TEST_MODULE);
        assert_eq!(owned.owner(), Some(&TEST_MODULE));
    }

    #[test]
    fn test_mode_entry_id() {
        let entry = ModeEntry::from_mode(TestMode::Input);
        assert_eq!(entry.id(), &TestMode::Input.id());
    }

    #[test]
    fn test_mode_registry_get() {
        let mut registry = ModeRegistry::new();
        registry.register_mode(TestMode::Command);

        assert!(registry.get(&TestMode::Command.id()).is_some());
        assert!(registry.get(&TestMode::Input.id()).is_none());
    }

    #[test]
    fn test_mode_registry_has_selection() {
        let mut registry = ModeRegistry::new();
        registry.register_all([TestMode::Command, TestMode::Selection]);

        assert!(!registry.has_selection(&TestMode::Command.id()));
        assert!(registry.has_selection(&TestMode::Selection.id()));
        // Unregistered mode
        assert!(!registry.has_selection(&TestMode::Input.id()));
    }

    #[test]
    fn test_mode_registry_display_name() {
        let mut registry = ModeRegistry::new();
        registry.register_mode(TestMode::Command);

        assert_eq!(registry.display_name(&TestMode::Command.id()), "COMMAND");
        assert_eq!(registry.display_name(&TestMode::Input.id()), "UNKNOWN");
    }

    #[test]
    fn test_mode_registry_inherits_from() {
        let mut registry = ModeRegistry::new();
        registry.register_all([TestMode::Command, TestMode::Selection]);

        assert!(registry.inherits_from(&TestMode::Command.id()).is_none());
        assert!(registry.inherits_from(&TestMode::Selection.id()).is_some());
        assert!(registry.inherits_from(&TestMode::Input.id()).is_none());
    }

    #[test]
    fn test_mode_registry_cursor_style_unregistered() {
        let registry = ModeRegistry::new();
        // Unregistered mode should return Block
        assert_eq!(registry.cursor_style(&TestMode::Input.id()), CursorStyle::Block);
    }

    #[test]
    fn test_mode_registry_ids() {
        let mut registry = ModeRegistry::new();
        registry.register_all([TestMode::Command, TestMode::Input]);

        assert_eq!(registry.ids().count(), 2);
    }

    #[test]
    fn test_mode_registry_find_by_name() {
        let mut registry = ModeRegistry::new();
        registry.register_all([TestMode::Command, TestMode::Input]);

        let found = registry.find_by_name("test", "COMMAND");
        assert!(found.is_some());

        let not_found = registry.find_by_name("test", "nonexistent");
        assert!(not_found.is_none());

        let wrong_module = registry.find_by_name("wrong", "COMMAND");
        assert!(wrong_module.is_none());
    }

    #[test]
    fn test_mode_registry_len_is_empty() {
        let mut registry = ModeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register_mode(TestMode::Command);
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_mode_registry_unregister_for_module() {
        let mut registry = ModeRegistry::new();
        let entry = ModeEntry::from_mode(TestMode::Command).with_owner(TEST_MODULE);
        registry.register(entry);

        let entry2 = ModeEntry::from_mode(TestMode::Input); // no owner
        registry.register(entry2);

        assert_eq!(registry.len(), 2);
        let removed = registry.unregister_for_module(&TEST_MODULE);
        assert_eq!(removed, 1);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_mode_entry_from_info() {
        let info = reovim_driver_input::ModeInfo {
            id: TestMode::Command.id(),
            display_name: "TEST",
            cursor_style: CursorStyle::Underline,
            accepts_char_input: true,
            has_selection: false,
            inherits_from: None,
            is_entry: false,
        };
        let entry = ModeEntry::from_info(info);
        assert_eq!(entry.display_name, "TEST");
        assert_eq!(entry.cursor_style, CursorStyle::Underline);
        assert!(entry.accepts_char_input);
    }

    #[test]
    fn test_mode_registry_default() {
        let registry = ModeRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_mode_registry_debug() {
        let registry = ModeRegistry::new();
        let debug = format!("{registry:?}");
        assert!(debug.contains("ModeRegistry"));
    }

    #[test]
    fn test_mode_entry_first_entry_wins() {
        let mut registry = ModeRegistry::new();
        // Command is_entry = true, so it becomes the entry mode
        registry.register_mode(TestMode::Command);
        // Input is_entry = false, but Selection inherits from Command
        registry.register_mode(TestMode::Input);
        // Entry mode should still be Command (first registered entry mode)
        assert_eq!(registry.entry_mode(), Some(&TestMode::Command.id()));
    }
}
