//! Keybinding store for `ServiceRegistry`.

use std::sync::RwLock;

use reovim_kernel::api::v1::{KeybindingRegistration, Service};

/// Store for keybinding registrations by modules.
pub struct KeybindingStore {
    keybindings: RwLock<Vec<KeybindingRegistration>>,
}

impl KeybindingStore {
    /// Create a new empty keybinding store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new() -> Self {
        Self {
            keybindings: RwLock::new(Vec::new()),
        }
    }

    /// Add a keybinding to the store.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn add(&self, binding: KeybindingRegistration) {
        self.keybindings
            .write()
            .expect("KeybindingStore lock poisoned")
            .push(binding);
    }

    /// Add multiple keybindings to the store.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn add_all(&self, bindings: impl IntoIterator<Item = KeybindingRegistration>) {
        self.keybindings
            .write()
            .expect("KeybindingStore lock poisoned")
            .extend(bindings);
    }

    /// Take all keybindings, clearing the store.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
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
    /// Panics if the internal lock is poisoned.
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
    /// Panics if the internal lock is poisoned.
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

impl Service for KeybindingStore {}

impl std::fmt::Debug for KeybindingStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeybindingStore")
            .field("count", &self.len())
            .finish()
    }
}
