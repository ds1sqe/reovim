#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Command picker module for reovim.
//!
//! Provides a command palette for registered commands.
//! Registers `CommandsPicker` in the `PickerRegistry` during module init.

use std::sync::Arc;

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry, SessionRuntime,
    },
    reovim_driver_text_session::CommandApi,
    reovim_kernel::api::v1::{
        CommandId, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_subsys_command_types::CommandContext,
};

// ============================================================================
// CommandsPicker
// ============================================================================

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

    fn items(
        &self,
        ctx: &PickerContext,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Vec<PickerItem> {
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, action: PickerAction, runtime: &mut SessionRuntime<'_>) {
        if let PickerAction::ExecuteCommand(qualified) = action {
            let cmd = CommandId::from_qualified(qualified);
            let ctx = CommandContext::new();
            runtime.execute_command(cmd, ctx);
        }
    }
}

// ============================================================================
// Module implementation
// ============================================================================

/// Command picker module.
///
/// Registers `CommandsPicker` in `PickerRegistry` during init.
pub struct PickerCommandsModule;

impl PickerCommandsModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerCommandsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerCommandsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-commands")
    }

    fn name(&self) -> &'static str {
        "Command Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(CommandsPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerCommandsModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
