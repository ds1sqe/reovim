use crate::{PickerAction, PickerContext, PickerItem, PreviewContent};

/// Trait for pluggable data sources in the fuzzy finder.
///
/// Pickers define WHAT items are available and what happens when selected.
/// The fuzzy matching engine handles HOW they are filtered and ranked.
///
/// # Sync Design
///
/// `items()` is synchronous. For expensive data sources (file walk, grep),
/// the caller should spawn a background task that feeds items into the
/// engine's `Injector` directly, bypassing `items()`.
pub trait Picker: Send + Sync {
    /// Unique name for this picker (e.g. "files", "buffers").
    fn name(&self) -> &'static str;

    /// Human-readable title for the picker UI.
    fn title(&self) -> &'static str;

    /// Prompt prefix (shown before the query input).
    fn prompt(&self) -> &'static str {
        "> "
    }

    /// Produce items to search through.
    ///
    /// For static sources (buffers, commands), returns all items.
    /// For dynamic sources (grep), returns results for current query.
    fn items(&self, ctx: &PickerContext) -> Vec<PickerItem>;

    /// Resolve the action when a user selects an item.
    fn on_select(&self, item: &PickerItem) -> PickerAction;

    /// Optional: provide preview content for the highlighted item.
    fn preview(&self, _item: &PickerItem) -> Option<PreviewContent> {
        None
    }

    /// Whether this picker supports client-side fuzzy filtering.
    ///
    /// If true (default), items are fetched once and nucleo handles filtering.
    /// If false, the picker needs re-fetching on each query change (e.g. live grep).
    fn is_static(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use crate::PickerData;

    use super::*;

    struct MockPicker;

    impl Picker for MockPicker {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn title(&self) -> &'static str {
            "Mock Picker"
        }

        fn items(&self, ctx: &PickerContext) -> Vec<PickerItem> {
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

        fn items(&self, _ctx: &PickerContext) -> Vec<PickerItem> {
            vec![]
        }

        fn on_select(&self, _item: &PickerItem) -> PickerAction {
            PickerAction::Close
        }

        fn preview(&self, _item: &PickerItem) -> Option<PreviewContent> {
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
        };
        let items = picker.items(&ctx);
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
        };
        let items = picker.items(&ctx);
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
        assert!(picker.preview(&item).is_none());
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
        let preview = picker.preview(&item);
        assert!(preview.is_some());
        let preview = preview.unwrap();
        assert_eq!(preview.lines.len(), 1);
        assert_eq!(preview.highlight_line, Some(0));
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
}
