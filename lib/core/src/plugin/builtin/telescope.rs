//! Telescope fuzzy finder plugin

use std::any::TypeId;

use crate::{
    command::builtin::{
        TelescopeBackspaceCommand, TelescopeCloseCommand, TelescopeCommandsCommand,
        TelescopeConfirmCommand, TelescopeEnterInsertCommand, TelescopeEnterNormalCommand,
        TelescopeFindBuffersCommand, TelescopeFindFilesCommand, TelescopeGotoFirstCommand,
        TelescopeGotoLastCommand, TelescopeHelpTagsCommand, TelescopeKeymapsCommand,
        TelescopeLiveGrepCommand, TelescopePageDownCommand, TelescopePageUpCommand,
        TelescopeRecentFilesCommand, TelescopeSelectNextCommand, TelescopeSelectPrevCommand,
        TelescopeThemesCommand,
    },
    plugin::{Plugin, PluginContext, PluginId},
    telescope::picker::{
        BuffersPicker, CommandsPicker, FilesPicker, GrepPicker, HelpPicker, KeymapsPicker,
        ProfilesPicker, RecentPicker, ThemesPicker,
    },
};

use super::CorePlugin;

/// Telescope fuzzy finder plugin
///
/// Provides fuzzy finding capabilities:
/// - File picker (Space ff)
/// - Buffer picker (Space fb)
/// - Grep picker (Space fg)
/// - Command palette
/// - And more...
pub struct TelescopePlugin;

impl Plugin for TelescopePlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:telescope")
    }

    fn name(&self) -> &'static str {
        "Telescope"
    }

    fn description(&self) -> &'static str {
        "Fuzzy finder: files, buffers, grep, commands"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        self.register_commands(ctx);
        self.register_pickers(ctx);
    }
}

#[allow(clippy::unused_self)]
impl TelescopePlugin {
    fn register_commands(&self, ctx: &PluginContext) {
        // Picker launchers
        let _ = ctx.register_command(TelescopeFindFilesCommand);
        let _ = ctx.register_command(TelescopeFindBuffersCommand);
        let _ = ctx.register_command(TelescopeLiveGrepCommand);
        let _ = ctx.register_command(TelescopeRecentFilesCommand);
        let _ = ctx.register_command(TelescopeCommandsCommand);
        let _ = ctx.register_command(TelescopeHelpTagsCommand);
        let _ = ctx.register_command(TelescopeKeymapsCommand);
        let _ = ctx.register_command(TelescopeThemesCommand);

        // Navigation
        let _ = ctx.register_command(TelescopeSelectNextCommand);
        let _ = ctx.register_command(TelescopeSelectPrevCommand);
        let _ = ctx.register_command(TelescopePageDownCommand);
        let _ = ctx.register_command(TelescopePageUpCommand);
        let _ = ctx.register_command(TelescopeGotoFirstCommand);
        let _ = ctx.register_command(TelescopeGotoLastCommand);

        // Actions
        let _ = ctx.register_command(TelescopeConfirmCommand);
        let _ = ctx.register_command(TelescopeCloseCommand);
        let _ = ctx.register_command(TelescopeBackspaceCommand);
        let _ = ctx.register_command(TelescopeEnterInsertCommand);
        let _ = ctx.register_command(TelescopeEnterNormalCommand);
    }

    fn register_pickers(&self, ctx: &mut PluginContext) {
        ctx.register_picker(FilesPicker::new());
        ctx.register_picker(BuffersPicker::new());
        ctx.register_picker(CommandsPicker::new());
        ctx.register_picker(KeymapsPicker::new());
        ctx.register_picker(GrepPicker::new());
        ctx.register_picker(RecentPicker::new());
        ctx.register_picker(HelpPicker::new());
        ctx.register_picker(ThemesPicker::new());
        ctx.register_picker(ProfilesPicker::new());
    }
}
