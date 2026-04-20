use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use reovim_kernel::api::Service;

use crate::Picker;

/// Custom registry for picker implementations.
///
/// Uses string keys (`picker.name()`) for open-ended extensibility.
/// Any module can register new pickers without modifying the driver crate.
///
/// Stored in `ServiceRegistry` via the `Service` marker trait.
pub struct PickerRegistry {
    pickers: RwLock<HashMap<&'static str, Arc<dyn Picker>>>,
}

impl Service for PickerRegistry {}

impl PickerRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pickers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a picker. Key is `picker.name()`.
    ///
    /// If a picker with the same name already exists, it is replaced.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn register(&self, picker: Arc<dyn Picker>) {
        self.pickers
            .write()
            .expect("PickerRegistry lock poisoned")
            .insert(picker.name(), picker);
    }

    /// Get a picker by name.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Picker>> {
        self.pickers
            .read()
            .expect("PickerRegistry lock poisoned")
            .get(name)
            .cloned()
    }

    /// List all registered picker names.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn list(&self) -> Vec<&'static str> {
        self.pickers
            .read()
            .expect("PickerRegistry lock poisoned")
            .keys()
            .copied()
            .collect()
    }

    /// Number of registered pickers.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pickers
            .read()
            .expect("PickerRegistry lock poisoned")
            .len()
    }

    /// Whether the registry is empty.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pickers
            .read()
            .expect("PickerRegistry lock poisoned")
            .is_empty()
    }
}

impl Default for PickerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
