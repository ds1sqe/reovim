//! Command palette picker implementation

use std::{future::Future, pin::Pin};

use {
    reovim_core::command::CommandId,
    reovim_plugin_microscope::{
        MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
    },
};

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
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        Box::pin(async move {
            self.commands
                .iter()
                .map(|cmd| {
                    MicroscopeItem::new(
                        cmd.id.as_str(),
                        cmd.id.as_str(),
                        MicroscopeData::Command(cmd.id.clone()),
                        "commands",
                    )
                    .with_detail(&cmd.description)
                })
                .collect()
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::Command(id) => MicroscopeAction::ExecuteCommand(id.clone()),
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
        _ctx: &PickerContext,
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
