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
