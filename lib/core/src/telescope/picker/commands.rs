//! Command palette picker implementation

use std::future::Future;
use std::pin::Pin;

use crate::command::CommandId;
use crate::telescope::item::{TelescopeData, TelescopeItem};
use crate::telescope::state::PreviewContent;

use super::{Picker, PickerContext, TelescopeAction};

/// Command info for the command palette
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Command ID
    pub id: CommandId,
    /// Command description
    pub description: String,
}

/// Picker for the command palette
pub struct CommandsPicker {
    /// Available commands (set by runtime from registry)
    commands: Vec<CommandInfo>,
}

impl CommandsPicker {
    /// Create a new commands picker
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Set available commands
    pub fn set_commands(&mut self, commands: Vec<CommandInfo>) {
        self.commands = commands;
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
        "Command Palette"
    }

    fn prompt(&self) -> &'static str {
        "Commands> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<TelescopeItem>> + Send + '_>> {
        Box::pin(async move {
            self.commands
                .iter()
                .map(|cmd| {
                    TelescopeItem::new(
                        cmd.id.as_str(),
                        cmd.id.as_str(),
                        TelescopeData::Command(cmd.id.clone()),
                        "commands",
                    )
                    .with_detail(&cmd.description)
                })
                .collect()
        })
    }

    fn on_select(&self, item: &TelescopeItem) -> TelescopeAction {
        match &item.data {
            TelescopeData::Command(id) => TelescopeAction::ExecuteCommand(id.clone()),
            _ => TelescopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &TelescopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let description = item.detail.clone();
        let display = item.display.clone();

        Box::pin(async move {
            description.map(|desc| {
                PreviewContent::new(vec![
                    format!("Command: {}", display),
                    String::new(),
                    format!("Description: {}", desc),
                ])
            })
        })
    }
}
