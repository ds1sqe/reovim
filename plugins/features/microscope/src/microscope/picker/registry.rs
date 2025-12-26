//! Picker Registry for dynamic picker registration
//!
//! This module provides `PickerRegistry` which allows plugins to register
//! custom pickers at runtime. The registry is stored in `PluginStateRegistry`
//! and can be accessed by any plugin.

use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;

use super::Picker;

/// Registry for dynamically registered pickers
///
/// This allows other plugins to register custom pickers without
/// modifying the microscope plugin directly.
///
/// # Example
///
/// ```ignore
/// // In your plugin's init_state():
/// registry.with_mut::<PickerRegistry, _, _>(|picker_registry| {
///     picker_registry.register(Arc::new(MyCustomPicker));
/// });
/// ```
#[derive(Default)]
pub struct PickerRegistry {
    pickers: RwLock<HashMap<String, Arc<dyn Picker>>>,
}

impl PickerRegistry {
    /// Create a new empty picker registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            pickers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a picker
    ///
    /// If a picker with the same name already exists, it will be replaced.
    pub fn register(&self, picker: Arc<dyn Picker>) {
        let name = picker.name().to_string();
        self.pickers.write().insert(name, picker);
    }

    /// Get a picker by name
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Picker>> {
        self.pickers.read().get(name).cloned()
    }

    /// Check if a picker with the given name exists
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.pickers.read().contains_key(name)
    }

    /// Get all registered picker names
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        self.pickers.read().keys().cloned().collect()
    }

    /// Get the number of registered pickers
    #[must_use]
    pub fn len(&self) -> usize {
        self.pickers.read().len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pickers.read().is_empty()
    }

    /// Remove a picker by name
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn Picker>> {
        self.pickers.write().remove(name)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::microscope::{
            item::{MicroscopeData, MicroscopeItem},
            state::PreviewContent,
        },
        std::{future::Future, path::PathBuf, pin::Pin},
    };

    /// Mock picker for testing
    struct MockPicker;

    impl super::super::Picker for MockPicker {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn title(&self) -> &'static str {
            "Mock Picker"
        }

        fn fetch(
            &self,
            _ctx: &super::super::PickerContext,
        ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
            Box::pin(async {
                vec![MicroscopeItem::new(
                    "test",
                    "Test Item",
                    MicroscopeData::FilePath(PathBuf::from("test.txt")),
                    "mock",
                )]
            })
        }

        fn on_select(&self, _item: &MicroscopeItem) -> super::super::MicroscopeAction {
            super::super::MicroscopeAction::Nothing
        }

        fn preview(
            &self,
            _item: &MicroscopeItem,
        ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
            Box::pin(async { None })
        }
    }

    #[test]
    fn test_register_and_get() {
        let registry = PickerRegistry::new();
        let picker = Arc::new(MockPicker);

        registry.register(picker);

        assert!(registry.contains("mock"));
        assert!(registry.get("mock").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_pickers() {
        let registry = PickerRegistry::new();
        registry.register(Arc::new(MockPicker));

        let names = registry.list();
        assert!(names.contains(&"mock".to_string()));
    }

    #[test]
    fn test_unregister() {
        let registry = PickerRegistry::new();
        registry.register(Arc::new(MockPicker));

        assert!(registry.contains("mock"));
        registry.unregister("mock");
        assert!(!registry.contains("mock"));
    }
}
