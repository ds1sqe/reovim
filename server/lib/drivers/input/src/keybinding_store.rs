//! Keybinding store for `ServiceRegistry`.
//!
//! This module provides a store for keybinding registrations that modules can
//! register during `init()`. The runner extracts this after all modules
//! initialize to populate its `KeymapRegistry`.
//!
//! # Architecture
//!
//! Following the Epic #417 pattern:
//! - **Mechanism (driver)**: This store type
//! - **Policy (modules)**: Register their keybindings during `init()`
//!
//! # Example
//!
//! ```ignore
//! // In VimModule::init():
//! let store = ctx.services.get_or_create::<KeybindingStore>();
//! for binding in self.keybindings() {
//!     store.add(binding);
//! }
//!
//! // In runner after all modules initialized:
//! let store = services.get::<KeybindingStore>().unwrap();
//! for binding in store.take_keybindings() {
//!     keymap_registry.register(binding);
//! }
//! ```

use std::sync::RwLock;

use reovim_kernel::api::v1::{KeybindingRegistration, Service};

/// Store for keybinding registrations by modules.
///
/// Modules register their keybindings during `init()` by calling `add()`.
/// After all modules are initialized, the runner extracts keybindings
/// via `take_keybindings()`.
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability.
pub struct KeybindingStore {
    keybindings: RwLock<Vec<KeybindingRegistration>>,
}

impl KeybindingStore {
    /// Create a new empty keybinding store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new() is not const
    pub fn new() -> Self {
        Self {
            keybindings: RwLock::new(Vec::new()),
        }
    }

    /// Add a keybinding to the store.
    ///
    /// Called by modules during `init()`.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add(&self, binding: KeybindingRegistration) {
        self.keybindings
            .write()
            .expect("KeybindingStore lock poisoned")
            .push(binding);
    }

    /// Add multiple keybindings to the store.
    ///
    /// Convenience method for adding a batch of bindings.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add_all(&self, bindings: impl IntoIterator<Item = KeybindingRegistration>) {
        self.keybindings
            .write()
            .expect("KeybindingStore lock poisoned")
            .extend(bindings);
    }

    /// Take all keybindings, clearing the store.
    ///
    /// Called by runner after all modules are initialized.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn take_keybindings(&self) -> Vec<KeybindingRegistration> {
        std::mem::take(
            &mut *self
                .keybindings
                .write()
                .expect("KeybindingStore lock poisoned"),
        )
    }

    /// Get the number of registered keybindings.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keybindings
            .read()
            .expect("KeybindingStore lock poisoned")
            .len()
    }

    /// Check if the store is empty.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keybindings
            .read()
            .expect("KeybindingStore lock poisoned")
            .is_empty()
    }
}

impl Default for KeybindingStore {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Service so KeybindingStore can be stored in ServiceRegistry
impl Service for KeybindingStore {}

impl std::fmt::Debug for KeybindingStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeybindingStore")
            .field("count", &self.len())
            .finish()
    }
}

