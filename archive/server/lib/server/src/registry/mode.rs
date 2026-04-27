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

    /// Create a mode entry directly from field values.
    ///
    /// Used by the composition root (bootstrap) when converting driver-layer
    /// `ModeInfo` structs into server-side `ModeEntry` instances.  The server
    /// does NOT import driver-tier `ModeInfo` directly; the caller extracts
    /// the fields and passes them here.
    #[must_use]
    #[allow(clippy::fn_params_excessive_bools)] // five bool fields mirror ModeInfo shape
    pub const fn from_fields(
        id: ModeId,
        display_name: &'static str,
        cursor_style: CursorStyle,
        accepts_char_input: bool,
        has_selection: bool,
        inherits_from: Option<ModeId>,
        is_entry: bool,
    ) -> Self {
        Self {
            id,
            display_name,
            cursor_style,
            accepts_char_input,
            has_selection,
            inherits_from,
            is_entry,
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
#[path = "mode_tests.rs"]
mod tests;
