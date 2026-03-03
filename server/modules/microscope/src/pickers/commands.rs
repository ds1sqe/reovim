//! Commands picker - command palette for registered commands.

use reovim_driver_picker::{Picker, PickerAction, PickerContext, PickerData, PickerItem};

/// Picker that lists registered commands.
///
/// Reads `ctx.commands` and produces items for each command.
/// Selecting a command executes it via `PickerAction::ExecuteCommand`.
pub struct CommandsPicker;

impl CommandsPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CommandsPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for CommandsPicker {
    fn name(&self) -> &'static str {
        "commands"
    }

    fn title(&self) -> &'static str {
        "Commands"
    }

    fn items(&self, ctx: &PickerContext) -> Vec<PickerItem> {
        ctx.commands
            .iter()
            .map(|cmd| PickerItem {
                display: cmd.qualified_name.clone(),
                detail: Some(cmd.description.clone()),
                data: PickerData::Command(cmd.qualified_name.clone()),
                icon: None,
            })
            .collect()
    }

    fn on_select(&self, item: &PickerItem) -> PickerAction {
        match &item.data {
            PickerData::Command(name) => PickerAction::ExecuteCommand(name.clone()),
            _ => PickerAction::Close,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use reovim_driver_picker::CommandInfo;

    use super::*;

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
        let picker = CommandsPicker::new();
        assert_eq!(picker.name(), "commands");
        assert_eq!(picker.title(), "Commands");
    }

    #[test]
    fn is_static() {
        let picker = CommandsPicker::new();
        assert!(picker.is_static());
    }

    #[test]
    fn default_prompt() {
        let picker = CommandsPicker::new();
        assert_eq!(picker.prompt(), "> ");
    }

    #[test]
    fn items_empty_context() {
        let picker = CommandsPicker::new();
        let items = picker.items(&empty_ctx());
        assert!(items.is_empty());
    }

    #[test]
    fn items_from_commands() {
        let picker = CommandsPicker::new();
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![
                CommandInfo {
                    qualified_name: "editor:save".to_owned(),
                    description: "Save the current file".to_owned(),
                },
                CommandInfo {
                    qualified_name: "editor:quit".to_owned(),
                    description: "Quit the editor".to_owned(),
                },
            ],
        };

        let items = picker.items(&ctx);
        assert_eq!(items.len(), 2);

        assert_eq!(items[0].display, "editor:save");
        assert_eq!(items[0].detail.as_deref(), Some("Save the current file"));
        assert!(items[0].icon.is_none());

        assert_eq!(items[1].display, "editor:quit");
        assert_eq!(items[1].detail.as_deref(), Some("Quit the editor"));
    }

    #[test]
    fn on_select_command() {
        let picker = CommandsPicker::new();
        let item = PickerItem {
            display: "editor:save".to_owned(),
            detail: None,
            data: PickerData::Command("editor:save".to_owned()),
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(matches!(action, PickerAction::ExecuteCommand(ref name) if name == "editor:save"));
    }

    #[test]
    fn on_select_wrong_data_closes() {
        let picker = CommandsPicker::new();
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
        let picker = CommandsPicker::default();
        assert_eq!(picker.name(), "commands");
    }

    #[test]
    fn no_preview() {
        let picker = CommandsPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::Command("x".to_owned()),
            icon: None,
        };
        assert!(picker.preview(&item).is_none());
    }
}
