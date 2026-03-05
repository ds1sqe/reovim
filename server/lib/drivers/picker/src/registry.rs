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
mod tests {
    use {
        crate::{PickerAction, PickerContext, PickerItem},
        reovim_kernel::api::v1::ServiceRegistry,
    };

    use super::*;

    struct TestPicker {
        picker_name: &'static str,
    }

    impl Picker for TestPicker {
        fn name(&self) -> &'static str {
            self.picker_name
        }

        fn title(&self) -> &'static str {
            "Test"
        }

        fn items(&self, _ctx: &PickerContext, _services: &ServiceRegistry) -> Vec<PickerItem> {
            vec![]
        }

        fn on_select(&self, _item: &PickerItem) -> PickerAction {
            PickerAction::Close
        }
    }

    #[test]
    fn new_registry_is_empty() {
        let registry = PickerRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn default_registry_is_empty() {
        let registry = PickerRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn register_and_get() {
        let registry = PickerRegistry::new();
        let picker: Arc<dyn Picker> = Arc::new(TestPicker {
            picker_name: "files",
        });
        registry.register(picker);

        let got = registry.get("files");
        assert!(got.is_some());
        assert_eq!(got.unwrap().name(), "files");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let registry = PickerRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn register_multiple() {
        let registry = PickerRegistry::new();
        registry.register(Arc::new(TestPicker {
            picker_name: "files",
        }));
        registry.register(Arc::new(TestPicker {
            picker_name: "buffers",
        }));
        registry.register(Arc::new(TestPicker {
            picker_name: "grep",
        }));

        assert_eq!(registry.len(), 3);
        assert!(!registry.is_empty());

        let mut names = registry.list();
        names.sort_unstable();
        assert_eq!(names, vec!["buffers", "files", "grep"]);
    }

    #[test]
    fn register_duplicate_replaces() {
        let registry = PickerRegistry::new();
        registry.register(Arc::new(TestPicker {
            picker_name: "files",
        }));
        registry.register(Arc::new(TestPicker {
            picker_name: "files",
        }));

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn list_empty_registry() {
        let registry = PickerRegistry::new();
        assert!(registry.list().is_empty());
    }

    // Verify Service trait is implemented (compile-time check).
    fn assert_service(_: &dyn Service) {}

    #[test]
    fn registry_implements_service() {
        let registry = PickerRegistry::new();
        assert_service(&registry);
    }
}
