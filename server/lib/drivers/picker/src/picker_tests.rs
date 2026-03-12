use std::{path::PathBuf, sync::Arc};

use crate::PickerData;

use super::*;

fn services() -> ServiceRegistry {
    ServiceRegistry::new()
}

struct MockPicker;

impl Picker for MockPicker {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn title(&self) -> &'static str {
        "Mock Picker"
    }

    fn items(&self, ctx: &PickerContext, _services: &ServiceRegistry) -> Vec<PickerItem> {
        if ctx.query.is_empty() {
            vec![PickerItem {
                display: "item1".to_owned(),
                detail: None,
                data: PickerData::Text("data1".to_owned()),
                icon: None,
            }]
        } else {
            vec![]
        }
    }

    fn on_select(&self, _item: &PickerItem) -> PickerAction {
        PickerAction::Close
    }
}

struct CustomPromptPicker;

impl Picker for CustomPromptPicker {
    fn name(&self) -> &'static str {
        "custom"
    }

    fn title(&self) -> &'static str {
        "Custom"
    }

    fn prompt(&self) -> &'static str {
        "rg> "
    }

    fn items(&self, _ctx: &PickerContext, _services: &ServiceRegistry) -> Vec<PickerItem> {
        vec![]
    }

    fn on_select(&self, _item: &PickerItem) -> PickerAction {
        PickerAction::Close
    }

    fn preview(&self, _item: &PickerItem, _services: &ServiceRegistry) -> Option<PreviewContent> {
        Some(PreviewContent {
            lines: vec!["preview line".to_owned()],
            highlight_line: Some(0),
            file_path: None,
        })
    }

    fn is_static(&self) -> bool {
        false
    }
}

#[test]
fn mock_picker_name_and_title() {
    let picker = MockPicker;
    assert_eq!(picker.name(), "mock");
    assert_eq!(picker.title(), "Mock Picker");
}

#[test]
fn mock_picker_default_prompt() {
    let picker = MockPicker;
    assert_eq!(picker.prompt(), "> ");
}

#[test]
fn mock_picker_items_empty_query() {
    let picker = MockPicker;
    let ctx = PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    };
    let items = picker.items(&ctx, &services());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].display, "item1");
}

#[test]
fn mock_picker_items_with_query() {
    let picker = MockPicker;
    let ctx = PickerContext {
        cwd: PathBuf::from("."),
        query: "search".to_owned(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    };
    let items = picker.items(&ctx, &services());
    assert!(items.is_empty());
}

#[test]
fn mock_picker_on_select() {
    let picker = MockPicker;
    let item = PickerItem {
        display: "test".to_owned(),
        detail: None,
        data: PickerData::Text("x".to_owned()),
        icon: None,
    };
    let action = picker.on_select(&item);
    assert!(matches!(action, PickerAction::Close));
}

#[test]
fn mock_picker_default_preview() {
    let picker = MockPicker;
    let item = PickerItem {
        display: "test".to_owned(),
        detail: None,
        data: PickerData::Text("x".to_owned()),
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

#[test]
fn mock_picker_default_is_static() {
    let picker = MockPicker;
    assert!(picker.is_static());
}

#[test]
fn custom_picker_overrides() {
    let picker = CustomPromptPicker;
    assert_eq!(picker.prompt(), "rg> ");
    assert!(!picker.is_static());

    let item = PickerItem {
        display: "test".to_owned(),
        detail: None,
        data: PickerData::Text("x".to_owned()),
        icon: None,
    };
    let preview = picker.preview(&item, &services());
    assert!(preview.is_some());
    let preview = preview.unwrap();
    assert_eq!(preview.lines.len(), 1);
    assert_eq!(preview.highlight_line, Some(0));
}

#[test]
fn mock_picker_default_execute() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let picker = MockPicker;
    let mut test = TestSessionRuntime::new();
    let mut runtime = test.runtime();
    // Default execute() is a no-op — just verify it doesn't panic.
    picker.execute(PickerAction::Close, &mut runtime);
}

#[test]
fn trait_object_safety() {
    let picker: Arc<dyn Picker> = Arc::new(MockPicker);
    assert_eq!(picker.name(), "mock");
    assert_eq!(picker.title(), "Mock Picker");
    assert!(picker.is_static());
}

#[test]
fn trait_object_in_box() {
    let picker: Box<dyn Picker> = Box::new(CustomPromptPicker);
    assert_eq!(picker.name(), "custom");
    assert_eq!(picker.prompt(), "rg> ");
}
