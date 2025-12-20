//! Settings menu plugin

use std::any::TypeId;

use crate::{
    command::builtin::{
        SettingsMenuCloseCommand, SettingsMenuCycleNextCommand, SettingsMenuCyclePrevCommand,
        SettingsMenuDecrementCommand, SettingsMenuExecuteCommand, SettingsMenuIncrementCommand,
        SettingsMenuNextCommand, SettingsMenuOpenCommand, SettingsMenuPrevCommand,
        SettingsMenuQuick1Command, SettingsMenuQuick2Command, SettingsMenuQuick3Command,
        SettingsMenuQuick4Command, SettingsMenuQuick5Command, SettingsMenuQuick6Command,
        SettingsMenuQuick7Command, SettingsMenuQuick8Command, SettingsMenuQuick9Command,
        SettingsMenuToggleCommand,
    },
    plugin::{Plugin, PluginContext, PluginId},
};

use super::CorePlugin;

/// Settings menu plugin
///
/// Provides in-editor settings UI:
/// - Toggle settings
/// - Cycle through options
/// - Increment/decrement values
/// - Quick access keys (1-9)
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:settings")
    }

    fn name(&self) -> &'static str {
        "Settings"
    }

    fn description(&self) -> &'static str {
        "In-editor settings menu"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        self.register_navigation_commands(ctx);
        self.register_action_commands(ctx);
        self.register_quick_commands(ctx);
    }
}

#[allow(clippy::unused_self)]
impl SettingsPlugin {
    fn register_navigation_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(SettingsMenuOpenCommand);
        let _ = ctx.register_command(SettingsMenuCloseCommand);
        let _ = ctx.register_command(SettingsMenuNextCommand);
        let _ = ctx.register_command(SettingsMenuPrevCommand);
    }

    fn register_action_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(SettingsMenuToggleCommand);
        let _ = ctx.register_command(SettingsMenuCycleNextCommand);
        let _ = ctx.register_command(SettingsMenuCyclePrevCommand);
        let _ = ctx.register_command(SettingsMenuIncrementCommand);
        let _ = ctx.register_command(SettingsMenuDecrementCommand);
        let _ = ctx.register_command(SettingsMenuExecuteCommand);
    }

    fn register_quick_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(SettingsMenuQuick1Command);
        let _ = ctx.register_command(SettingsMenuQuick2Command);
        let _ = ctx.register_command(SettingsMenuQuick3Command);
        let _ = ctx.register_command(SettingsMenuQuick4Command);
        let _ = ctx.register_command(SettingsMenuQuick5Command);
        let _ = ctx.register_command(SettingsMenuQuick6Command);
        let _ = ctx.register_command(SettingsMenuQuick7Command);
        let _ = ctx.register_command(SettingsMenuQuick8Command);
        let _ = ctx.register_command(SettingsMenuQuick9Command);
    }
}
