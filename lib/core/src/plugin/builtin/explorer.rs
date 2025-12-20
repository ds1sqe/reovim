//! File explorer plugin

use std::any::TypeId;

use crate::{
    command::builtin::{
        ExplorerCancelInputCommand, ExplorerClearFilterCommand, ExplorerCloseCommand,
        ExplorerCloseParentCommand, ExplorerConfirmInputCommand, ExplorerCreateDirCommand,
        ExplorerCreateFileCommand, ExplorerCursorDownCommand, ExplorerCursorUpCommand,
        ExplorerCutCommand, ExplorerDeleteCommand, ExplorerExitVisualCommand,
        ExplorerFilterCommand, ExplorerFocusEditorCommand, ExplorerGoToParentCommand,
        ExplorerGotoFirstCommand, ExplorerGotoLastCommand, ExplorerInputBackspaceCommand,
        ExplorerOpenNodeCommand, ExplorerPageDownCommand, ExplorerPageUpCommand,
        ExplorerPasteCommand, ExplorerRefreshCommand, ExplorerRenameCommand,
        ExplorerSelectAllCommand, ExplorerToggleHiddenCommand, ExplorerToggleNodeCommand,
        ExplorerToggleSelectCommand, ExplorerToggleSizesCommand, ExplorerVisualModeCommand,
        ExplorerYankCommand,
    },
    plugin::{Plugin, PluginContext, PluginId},
};

use super::CorePlugin;

/// File explorer plugin
///
/// Provides file browser sidebar:
/// - Tree navigation
/// - File/directory operations
/// - Copy/cut/paste
/// - Visual selection
pub struct ExplorerPlugin;

impl Plugin for ExplorerPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:explorer")
    }

    fn name(&self) -> &'static str {
        "Explorer"
    }

    fn description(&self) -> &'static str {
        "File browser sidebar with tree navigation"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        self.register_navigation_commands(ctx);
        self.register_tree_commands(ctx);
        self.register_file_commands(ctx);
        self.register_clipboard_commands(ctx);
        self.register_visual_commands(ctx);
        self.register_input_commands(ctx);
    }
}

#[allow(clippy::unused_self)]
impl ExplorerPlugin {
    fn register_navigation_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerCursorUpCommand);
        let _ = ctx.register_command(ExplorerCursorDownCommand);
        let _ = ctx.register_command(ExplorerPageUpCommand);
        let _ = ctx.register_command(ExplorerPageDownCommand);
        let _ = ctx.register_command(ExplorerGotoFirstCommand);
        let _ = ctx.register_command(ExplorerGotoLastCommand);
        let _ = ctx.register_command(ExplorerGoToParentCommand);
    }

    fn register_tree_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerToggleNodeCommand);
        let _ = ctx.register_command(ExplorerOpenNodeCommand);
        let _ = ctx.register_command(ExplorerCloseParentCommand);
        let _ = ctx.register_command(ExplorerRefreshCommand);
        let _ = ctx.register_command(ExplorerToggleHiddenCommand);
        let _ = ctx.register_command(ExplorerToggleSizesCommand);
        let _ = ctx.register_command(ExplorerCloseCommand);
        let _ = ctx.register_command(ExplorerFocusEditorCommand);
    }

    fn register_file_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerCreateFileCommand);
        let _ = ctx.register_command(ExplorerCreateDirCommand);
        let _ = ctx.register_command(ExplorerRenameCommand);
        let _ = ctx.register_command(ExplorerDeleteCommand);
        let _ = ctx.register_command(ExplorerFilterCommand);
        let _ = ctx.register_command(ExplorerClearFilterCommand);
    }

    fn register_clipboard_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerYankCommand);
        let _ = ctx.register_command(ExplorerCutCommand);
        let _ = ctx.register_command(ExplorerPasteCommand);
    }

    fn register_visual_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerVisualModeCommand);
        let _ = ctx.register_command(ExplorerToggleSelectCommand);
        let _ = ctx.register_command(ExplorerSelectAllCommand);
        let _ = ctx.register_command(ExplorerExitVisualCommand);
    }

    fn register_input_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerConfirmInputCommand);
        let _ = ctx.register_command(ExplorerCancelInputCommand);
        let _ = ctx.register_command(ExplorerInputBackspaceCommand);
    }
}
