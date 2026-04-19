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

use reovim_kernel::api::v1::{Mode, ModeId, Service};

pub use reovim_subsys_input_contracts::ModeInfo;

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

    /// Find a registered mode by module name and mode name.
    ///
    /// Returns `Some(ModeId)` if a mode with the given module and name
    /// has been registered, `None` otherwise.
    ///
    /// This is intended for use during `Module::init()` to resolve
    /// parent modes for inheritance without hardcoding foreign constants.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn find_by_name(&self, module: &str, name: &str) -> Option<ModeId> {
        self.modes
            .read()
            .expect("ModeInfoStore lock poisoned")
            .iter()
            .find(|m| m.id.module().as_str() == module && m.id.name() == name)
            .map(|m| m.id.clone())
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
