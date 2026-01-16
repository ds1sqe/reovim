//! Mode registry for storing mode metadata and behavior.
//!
//! Modes in reovim follow the kernel's trait separation:
//! - `Mode` (kernel): Identity only (provides `ModeId`)
//! - `ModeDisplay` (display driver): Cursor style, status text
//! - `ModeInput` (input driver): Whether mode accepts char input
//!
//! This registry stores all three aspects together for easy lookup.
//!
//! # Module Ownership
//!
//! Modes can be registered with optional module ownership via
//! [`register_for_module`]. When a module is unloaded, all its
//! registered modes can be removed via [`unregister_for_module`].

use std::{collections::HashMap, sync::Arc};

use {
    reovim_driver_display::{CursorStyle, ModeDisplay},
    reovim_driver_input::ModeInput,
    reovim_kernel::api::v1::{Mode, ModeId, ModuleId},
};

/// Entry in the mode registry containing all mode aspects.
///
/// Bundles the mode's identity with its display and input behavior.
/// All fields are optional except the mode itself because some modes
/// might not implement all traits.
pub struct ModeEntry {
    /// The mode implementation (provides identity via `ModeId`).
    pub mode: Arc<dyn Mode>,

    /// Display behavior (cursor style, status text).
    ///
    /// `None` if the mode doesn't customize display.
    pub display: Option<Arc<dyn ModeDisplay>>,

    /// Input behavior (whether mode accepts character input).
    ///
    /// `None` if the mode doesn't customize input handling.
    pub input: Option<Arc<dyn ModeInput>>,

    /// The module that owns this mode (if any).
    ///
    /// `None` for built-in modes or modes registered without ownership.
    owner: Option<ModuleId>,
}

impl ModeEntry {
    /// Create a new mode entry with just the mode (no display/input).
    #[must_use]
    pub fn new(mode: Arc<dyn Mode>) -> Self {
        Self {
            mode,
            display: None,
            input: None,
            owner: None,
        }
    }

    /// Create a mode entry with display behavior.
    #[must_use]
    pub fn with_display(mut self, display: Arc<dyn ModeDisplay>) -> Self {
        self.display = Some(display);
        self
    }

    /// Create a mode entry with input behavior.
    #[must_use]
    pub fn with_input(mut self, input: Arc<dyn ModeInput>) -> Self {
        self.input = Some(input);
        self
    }

    /// Set the owning module for this mode entry.
    #[must_use]
    pub fn with_owner(mut self, owner: ModuleId) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Get the mode's ID.
    #[must_use]
    pub fn id(&self) -> ModeId {
        self.mode.id()
    }

    /// Get the owning module ID if any.
    #[must_use]
    pub const fn owner(&self) -> Option<&ModuleId> {
        self.owner.as_ref()
    }
}

impl std::fmt::Debug for ModeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeEntry")
            .field("id", &self.mode.id())
            .field("has_display", &self.display.is_some())
            .field("has_input", &self.input.is_some())
            .field("owner", &self.owner)
            .finish()
    }
}

/// Registry for mode metadata and behavior.
///
/// Stores [`ModeEntry`] instances keyed by [`ModeId`]. The event loop
/// uses this to look up cursor styles and input behavior for the
/// current mode.
///
/// # Example
///
/// ```ignore
/// use runner::registry::{ModeRegistry, ModeEntry};
/// use std::sync::Arc;
///
/// let mut registry = ModeRegistry::new();
///
/// // Register a mode with display behavior
/// let normal = EditorMode::Normal;
/// let entry = ModeEntry::new(Arc::new(normal))
///     .with_display(Arc::new(normal))
///     .with_input(Arc::new(normal));
/// registry.register(entry);
///
/// // Look up mode behavior
/// let mode_id = EditorMode::NORMAL_ID;
/// assert!(registry.accepts_char_input(&mode_id) == false);
/// assert!(registry.cursor_style(&mode_id) == CursorStyle::Block);
/// ```
#[derive(Default)]
pub struct ModeRegistry {
    modes: HashMap<ModeId, ModeEntry>,
}

impl ModeRegistry {
    /// Create a new empty mode registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a mode entry.
    ///
    /// If a mode with the same ID already exists, it is replaced.
    pub fn register(&mut self, entry: ModeEntry) {
        let id = entry.id();
        self.modes.insert(id, entry);
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
    /// Returns `false` if the mode isn't registered or doesn't have
    /// input behavior defined.
    #[must_use]
    pub fn accepts_char_input(&self, id: &ModeId) -> bool {
        self.modes
            .get(id)
            .and_then(|e| e.input.as_ref())
            .is_some_and(|i| i.accepts_char_input())
    }

    /// Get the cursor style for a mode.
    ///
    /// Returns `CursorStyle::Block` (default) if the mode isn't registered
    /// or doesn't have display behavior defined.
    #[must_use]
    pub fn cursor_style(&self, id: &ModeId) -> CursorStyle {
        self.modes
            .get(id)
            .and_then(|e| e.display.as_ref())
            .map_or(CursorStyle::Block, |d| d.cursor_style())
    }

    /// Get the status text for a mode.
    ///
    /// Returns an empty string if the mode isn't registered or doesn't
    /// have display behavior defined.
    #[must_use]
    pub fn status_text(&self, id: &ModeId) -> &str {
        self.modes
            .get(id)
            .and_then(|e| e.display.as_ref())
            .map_or("", |d| d.status_text())
    }

    /// Get all registered mode IDs.
    pub fn ids(&self) -> impl Iterator<Item = &ModeId> {
        self.modes.keys()
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

impl std::fmt::Debug for ModeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeRegistry")
            .field("count", &self.modes.len())
            .field("modes", &self.modes.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    // Test mode implementation
    struct TestMode {
        id: ModeId,
        accepts_input: bool,
    }

    impl Mode for TestMode {
        fn id(&self) -> ModeId {
            self.id.clone()
        }
    }

    impl ModeDisplay for TestMode {
        fn cursor_style(&self) -> CursorStyle {
            if self.accepts_input {
                CursorStyle::Bar
            } else {
                CursorStyle::Block
            }
        }

        fn status_text(&self) -> &'static str {
            if self.accepts_input {
                "INSERT"
            } else {
                "NORMAL"
            }
        }
    }

    impl ModeInput for TestMode {
        fn accepts_char_input(&self) -> bool {
            self.accepts_input
        }
    }

    fn normal_mode() -> TestMode {
        TestMode {
            id: ModeId::new(ModuleId::new("test"), "normal"),
            accepts_input: false,
        }
    }

    fn insert_mode() -> TestMode {
        TestMode {
            id: ModeId::new(ModuleId::new("test"), "insert"),
            accepts_input: true,
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
        let mode = normal_mode();
        let id = mode.id.clone();

        let entry = ModeEntry::new(Arc::new(mode));
        registry.register(entry);

        assert!(registry.contains(&id));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_mode_registry_accepts_char_input() {
        let mut registry = ModeRegistry::new();

        // Normal mode - doesn't accept input
        let normal = normal_mode();
        let normal_id = normal.id.clone();
        let normal_arc: Arc<TestMode> = Arc::new(normal);
        registry.register(ModeEntry::new(normal_arc.clone()).with_input(normal_arc));

        // Insert mode - accepts input
        let insert = insert_mode();
        let insert_id = insert.id.clone();
        let insert_arc: Arc<TestMode> = Arc::new(insert);
        registry.register(ModeEntry::new(insert_arc.clone()).with_input(insert_arc));

        assert!(!registry.accepts_char_input(&normal_id));
        assert!(registry.accepts_char_input(&insert_id));
    }

    #[test]
    fn test_mode_registry_cursor_style() {
        let mut registry = ModeRegistry::new();

        let normal = normal_mode();
        let normal_id = normal.id.clone();
        let normal_arc: Arc<TestMode> = Arc::new(normal);
        registry.register(ModeEntry::new(normal_arc.clone()).with_display(normal_arc));

        let insert = insert_mode();
        let insert_id = insert.id.clone();
        let insert_arc: Arc<TestMode> = Arc::new(insert);
        registry.register(ModeEntry::new(insert_arc.clone()).with_display(insert_arc));

        assert_eq!(registry.cursor_style(&normal_id), CursorStyle::Block);
        assert_eq!(registry.cursor_style(&insert_id), CursorStyle::Bar);
    }

    #[test]
    fn test_mode_registry_status_text() {
        let mut registry = ModeRegistry::new();

        let normal = normal_mode();
        let normal_id = normal.id.clone();
        let normal_arc: Arc<TestMode> = Arc::new(normal);
        registry.register(ModeEntry::new(normal_arc.clone()).with_display(normal_arc));

        assert_eq!(registry.status_text(&normal_id), "NORMAL");
    }

    #[test]
    fn test_mode_registry_unknown_mode() {
        let registry = ModeRegistry::new();
        let unknown_id = ModeId::new(ModuleId::new("unknown"), "mode");

        // Unknown modes should return defaults
        assert!(!registry.accepts_char_input(&unknown_id));
        assert_eq!(registry.cursor_style(&unknown_id), CursorStyle::Block);
        assert_eq!(registry.status_text(&unknown_id), "");
    }

    #[test]
    fn test_mode_entry_with_owner() {
        let mode = normal_mode();
        let owner = ModuleId::new("my-module");

        let entry = ModeEntry::new(Arc::new(mode)).with_owner(owner.clone());

        assert_eq!(entry.owner(), Some(&owner));
    }

    #[test]
    fn test_mode_entry_without_owner() {
        let mode = normal_mode();
        let entry = ModeEntry::new(Arc::new(mode));

        assert!(entry.owner().is_none());
    }

    #[test]
    fn test_mode_registry_unregister_for_module() {
        let mut registry = ModeRegistry::new();
        let owner = ModuleId::new("my-module");

        // Register modes with owner
        let normal = normal_mode();
        let normal_id = normal.id.clone();
        registry.register(ModeEntry::new(Arc::new(normal)).with_owner(owner.clone()));

        let insert = insert_mode();
        let insert_id = insert.id.clone();
        registry.register(ModeEntry::new(Arc::new(insert)).with_owner(owner.clone()));

        // Register one mode without owner
        let other_mode = TestMode {
            id: ModeId::new(ModuleId::new("test"), "visual"),
            accepts_input: false,
        };
        let visual_id = other_mode.id.clone();
        registry.register(ModeEntry::new(Arc::new(other_mode)));

        assert_eq!(registry.len(), 3);

        // Unregister module's modes
        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 2);
        assert_eq!(registry.len(), 1);
        assert!(!registry.contains(&normal_id));
        assert!(!registry.contains(&insert_id));
        assert!(registry.contains(&visual_id));
    }

    #[test]
    fn test_mode_registry_unregister_for_module_empty() {
        let mut registry = ModeRegistry::new();
        let owner = ModuleId::new("my-module");

        // Register mode without owner
        registry.register(ModeEntry::new(Arc::new(normal_mode())));

        // Try to unregister for a module that has no modes
        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 0);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_mode_registry_multiple_modules() {
        let mut registry = ModeRegistry::new();
        let module_x = ModuleId::new("module-a");
        let module_y = ModuleId::new("module-b");

        let first_mode = TestMode {
            id: ModeId::new(ModuleId::new("test"), "mode-a"),
            accepts_input: false,
        };
        let first_mode_id = first_mode.id.clone();
        registry.register(ModeEntry::new(Arc::new(first_mode)).with_owner(module_x.clone()));

        let second_mode = TestMode {
            id: ModeId::new(ModuleId::new("test"), "mode-b"),
            accepts_input: true,
        };
        let second_mode_id = second_mode.id.clone();
        registry.register(ModeEntry::new(Arc::new(second_mode)).with_owner(module_y));

        assert_eq!(registry.len(), 2);

        // Unload module A
        registry.unregister_for_module(&module_x);

        assert_eq!(registry.len(), 1);
        assert!(!registry.contains(&first_mode_id));
        assert!(registry.contains(&second_mode_id));
    }
}
