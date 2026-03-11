#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Command picker module for reovim.
//!
//! Provides a command palette for registered commands.
//! Registers `CommandsPicker` in the `PickerRegistry` during module init.

use std::sync::Arc;

use {
    reovim_driver_command_types::CommandContext,
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry, SessionRuntime,
    },
    reovim_driver_session::CommandApi,
    reovim_kernel::api::v1::{
        CommandId, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
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
            let cmd = CommandId::from_qualified_leaked(qualified);
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
mod tests {
    use std::path::PathBuf;

    use reovim_driver_picker::CommandInfo;

    use super::*;

    fn services() -> reovim_kernel::api::v1::ServiceRegistry {
        reovim_kernel::api::v1::ServiceRegistry::new()
    }

    fn empty_ctx() -> PickerContext {
        PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
            options: vec![],
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
        let items = picker.items(&empty_ctx(), &services());
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
            options: vec![],
        };

        let items = picker.items(&ctx, &services());
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
        assert!(picker.preview(&item, &services()).is_none());
    }

    // -- Module tests --

    #[test]
    fn module_id() {
        let module = PickerCommandsModule::new();
        assert_eq!(module.id().as_str(), "picker-commands");
    }

    #[test]
    fn module_name() {
        let module = PickerCommandsModule::new();
        assert_eq!(module.name(), "Command Picker");
    }

    #[test]
    fn module_version() {
        let module = PickerCommandsModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = PickerCommandsModule::default();
        assert_eq!(module.id().as_str(), "picker-commands");
    }

    #[test]
    fn module_exit() {
        let mut module = PickerCommandsModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn module_init_registers_picker() {
        let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let ctx = ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services.clone(),
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
        );

        let mut module = PickerCommandsModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let registry = services.get::<PickerRegistry>();
        assert!(registry.is_some());
        let reg = registry.unwrap();
        assert!(reg.get("commands").is_some());
    }
}
