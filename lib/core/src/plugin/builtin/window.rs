//! Window management plugin

use std::any::TypeId;

use crate::{
    command::builtin::{
        TabCloseCommand, TabNewCommand, TabNextCommand, TabPrevCommand, WindowCloseCommand,
        WindowEqualizeCommand, WindowFocusDownCommand, WindowFocusLeftCommand,
        WindowFocusOrSplitDownCommand, WindowFocusOrSplitLeftCommand,
        WindowFocusOrSplitRightCommand, WindowFocusOrSplitUpCommand, WindowFocusRightCommand,
        WindowFocusUpCommand, WindowMoveDownCommand, WindowMoveLeftCommand, WindowMoveRightCommand,
        WindowMoveUpCommand, WindowOnlyCommand, WindowSplitHorizontalCommand,
        WindowSplitVerticalCommand,
    },
    plugin::{Plugin, PluginContext, PluginId},
};

use super::CorePlugin;

/// Window management plugin
///
/// Provides window and tab management:
/// - Split windows (horizontal/vertical)
/// - Focus navigation
/// - Window movement
/// - Tab management
pub struct WindowPlugin;

impl Plugin for WindowPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:window")
    }

    fn name(&self) -> &'static str {
        "Window"
    }

    fn description(&self) -> &'static str {
        "Window splits, focus, tabs"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        self.register_focus_commands(ctx);
        self.register_split_commands(ctx);
        self.register_move_commands(ctx);
        self.register_tab_commands(ctx);
    }
}

#[allow(clippy::unused_self)]
impl WindowPlugin {
    fn register_focus_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(WindowFocusLeftCommand);
        let _ = ctx.register_command(WindowFocusDownCommand);
        let _ = ctx.register_command(WindowFocusUpCommand);
        let _ = ctx.register_command(WindowFocusRightCommand);
        let _ = ctx.register_command(WindowFocusOrSplitLeftCommand);
        let _ = ctx.register_command(WindowFocusOrSplitDownCommand);
        let _ = ctx.register_command(WindowFocusOrSplitUpCommand);
        let _ = ctx.register_command(WindowFocusOrSplitRightCommand);
    }

    fn register_split_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(WindowSplitHorizontalCommand);
        let _ = ctx.register_command(WindowSplitVerticalCommand);
        let _ = ctx.register_command(WindowCloseCommand);
        let _ = ctx.register_command(WindowOnlyCommand);
        let _ = ctx.register_command(WindowEqualizeCommand);
    }

    fn register_move_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(WindowMoveLeftCommand);
        let _ = ctx.register_command(WindowMoveDownCommand);
        let _ = ctx.register_command(WindowMoveUpCommand);
        let _ = ctx.register_command(WindowMoveRightCommand);
    }

    fn register_tab_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(TabNewCommand);
        let _ = ctx.register_command(TabCloseCommand);
        let _ = ctx.register_command(TabNextCommand);
        let _ = ctx.register_command(TabPrevCommand);
    }
}
