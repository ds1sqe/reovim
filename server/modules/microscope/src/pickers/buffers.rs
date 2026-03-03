//! Buffers picker - switch between open buffers.

use {
    reovim_driver_picker::{Picker, PickerAction, PickerContext, PickerData, PickerItem},
    reovim_kernel::api::v1::ServiceRegistry,
};

/// Picker that lists open buffers.
///
/// Reads `ctx.buffers` and produces items for each buffer.
/// Selecting a buffer switches to it via `PickerAction::SwitchBuffer`.
pub struct BuffersPicker;

impl BuffersPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BuffersPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for BuffersPicker {
    fn name(&self) -> &'static str {
        "buffers"
    }

    fn title(&self) -> &'static str {
        "Buffers"
    }

    fn items(&self, ctx: &PickerContext, _services: &ServiceRegistry) -> Vec<PickerItem> {
        ctx.buffers
            .iter()
            .map(|buf| {
                let icon = if buf.modified { Some('+') } else { None };
                PickerItem {
                    display: buf.name.clone(),
                    detail: Some(format!("#{}", buf.id)),
                    data: PickerData::BufferId(buf.id),
                    icon,
                }
            })
            .collect()
    }

    fn on_select(&self, item: &PickerItem) -> PickerAction {
        match &item.data {
            PickerData::BufferId(id) => PickerAction::SwitchBuffer(*id),
            _ => PickerAction::Close,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use reovim_driver_picker::BufferInfo;

    use super::*;

    fn services() -> ServiceRegistry {
        ServiceRegistry::new()
    }

    fn empty_ctx() -> PickerContext {
        PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
        }
    }

    #[test]
    fn name_and_title() {
        let picker = BuffersPicker::new();
        assert_eq!(picker.name(), "buffers");
        assert_eq!(picker.title(), "Buffers");
    }

    #[test]
    fn is_static() {
        let picker = BuffersPicker::new();
        assert!(picker.is_static());
    }

    #[test]
    fn default_prompt() {
        let picker = BuffersPicker::new();
        assert_eq!(picker.prompt(), "> ");
    }

    #[test]
    fn items_empty_context() {
        let picker = BuffersPicker::new();
        let items = picker.items(&empty_ctx(), &services());
        assert!(items.is_empty());
    }

    #[test]
    fn items_from_buffers() {
        let picker = BuffersPicker::new();
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![
                BufferInfo {
                    id: 1,
                    name: "main.rs".to_owned(),
                    modified: false,
                },
                BufferInfo {
                    id: 2,
                    name: "lib.rs".to_owned(),
                    modified: true,
                },
            ],
            commands: vec![],
        };

        let items = picker.items(&ctx, &services());
        assert_eq!(items.len(), 2);

        assert_eq!(items[0].display, "main.rs");
        assert_eq!(items[0].detail.as_deref(), Some("#1"));
        assert!(items[0].icon.is_none());

        assert_eq!(items[1].display, "lib.rs");
        assert_eq!(items[1].detail.as_deref(), Some("#2"));
        assert_eq!(items[1].icon, Some('+'));
    }

    #[test]
    fn on_select_buffer_id() {
        let picker = BuffersPicker::new();
        let item = PickerItem {
            display: "test.rs".to_owned(),
            detail: None,
            data: PickerData::BufferId(42),
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(matches!(action, PickerAction::SwitchBuffer(42)));
    }

    #[test]
    fn on_select_wrong_data_closes() {
        let picker = BuffersPicker::new();
        let item = PickerItem {
            display: "test".to_owned(),
            detail: None,
            data: PickerData::Text("wrong".to_owned()),
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(matches!(action, PickerAction::Close));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_impl() {
        let picker = BuffersPicker::default();
        assert_eq!(picker.name(), "buffers");
    }

    #[test]
    fn no_preview() {
        let picker = BuffersPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::BufferId(1),
            icon: None,
        };
        assert!(picker.preview(&item, &services()).is_none());
    }
}
