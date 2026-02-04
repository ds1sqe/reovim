//! Mode info store for `ServiceRegistry`.
//!
//! This module provides a store for mode information that modules can
//! register during `init()`. The runner extracts this after all modules
//! initialize to populate its `ModeRegistry`.
//!
//! # Architecture
//!
//! Following the Epic #417 pattern:
//! - **Mechanism (driver)**: This store type + `ModeInfo` struct
//! - **Policy (modules)**: Register their modes during `init()`
//!
//! # Example
//!
//! ```ignore
//! // In VimModule::init():
//! let store = ctx.services.get_or_create::<ModeInfoStore>();
//! for mode in VimMode::ALL {
//!     store.add(ModeInfo::from_mode(*mode));
//! }
//!
//! // In runner after all modules initialized:
//! let store = services.get::<ModeInfoStore>().unwrap();
//! for info in store.take_modes() {
//!     mode_registry.register(ModeEntry::from_info(info));
//! }
//! ```

use std::sync::RwLock;

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, Service};

/// Information about a mode for registration.
///
/// This is a driver-side struct that holds mode metadata.
/// Modules convert their `Mode` implementations to this struct
/// for registration via `ServiceRegistry`.
#[derive(Debug, Clone)]
pub struct ModeInfo {
    /// The mode ID.
    pub id: ModeId,
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
}

impl ModeInfo {
    /// Create mode info from a Mode implementation.
    pub fn from_mode<M: Mode>(mode: M) -> Self {
        Self {
            id: mode.id(),
            display_name: mode.display_name(),
            cursor_style: mode.cursor_style(),
            accepts_char_input: mode.accepts_char_input(),
            has_selection: mode.has_selection(),
            inherits_from: mode.inherits_from().map(|m| m.id()),
            is_entry: mode.is_entry(),
        }
    }
}

/// Store for mode information registered by modules.
///
/// Modules register their modes during `init()` by calling `add()`.
/// After all modules are initialized, the runner extracts modes
/// via `take_modes()`.
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability.
pub struct ModeInfoStore {
    modes: RwLock<Vec<ModeInfo>>,
}

impl ModeInfoStore {
    /// Create a new empty mode store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new() is not const
    pub fn new() -> Self {
        Self {
            modes: RwLock::new(Vec::new()),
        }
    }

    /// Add a mode to the store.
    ///
    /// Called by modules during `init()`.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add(&self, info: ModeInfo) {
        self.modes
            .write()
            .expect("ModeInfoStore lock poisoned")
            .push(info);
    }

    /// Add a mode from a Mode implementation.
    ///
    /// Convenience method that converts the mode to `ModeInfo`.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add_mode<M: Mode>(&self, mode: M) {
        self.add(ModeInfo::from_mode(mode));
    }

    /// Take all modes, clearing the store.
    ///
    /// Called by runner after all modules are initialized.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn take_modes(&self) -> Vec<ModeInfo> {
        std::mem::take(&mut *self.modes.write().expect("ModeInfoStore lock poisoned"))
    }

    /// Get the number of registered modes.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modes
            .read()
            .expect("ModeInfoStore lock poisoned")
            .len()
    }

    /// Check if the store is empty.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modes
            .read()
            .expect("ModeInfoStore lock poisoned")
            .is_empty()
    }
}

impl Default for ModeInfoStore {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Service so ModeInfoStore can be stored in ServiceRegistry
impl Service for ModeInfoStore {}

impl std::fmt::Debug for ModeInfoStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeInfoStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    // Mock mode for testing
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct TestMode;

    impl Mode for TestMode {
        fn module() -> ModuleId {
            ModuleId::new("test")
        }

        fn discriminant(&self) -> u16 {
            0
        }

        fn id(&self) -> ModeId {
            ModeId::new(ModuleId::new("test"), "test")
        }

        fn display_name(&self) -> &'static str {
            "TEST"
        }

        fn cursor_style(&self) -> CursorStyle {
            CursorStyle::Block
        }

        fn accepts_char_input(&self) -> bool {
            false
        }

        fn has_selection(&self) -> bool {
            false
        }

        fn inherits_from(&self) -> Option<Self> {
            None
        }

        fn is_entry(&self) -> bool {
            false
        }
    }

    #[test]
    fn test_store_new() {
        let store = ModeInfoStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_store_add_mode() {
        let store = ModeInfoStore::new();
        store.add_mode(TestMode);

        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn test_store_take_modes() {
        let store = ModeInfoStore::new();
        store.add_mode(TestMode);

        let modes = store.take_modes();
        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].display_name, "TEST");
        assert!(store.is_empty()); // Store should be empty after take
    }

    #[test]
    fn test_mode_info_from_mode() {
        let info = ModeInfo::from_mode(TestMode);
        assert_eq!(info.display_name, "TEST");
        assert!(!info.accepts_char_input);
        assert!(!info.has_selection);
    }
}
