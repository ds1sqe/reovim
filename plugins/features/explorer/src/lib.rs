//! File explorer plugin for reovim
//!
//! This plugin provides file browser functionality:
//! - Tree navigation with expand/collapse
//! - File/directory operations (create, rename, delete)
//! - Copy/cut/paste operations
//! - Visual selection mode
//! - Filter and search
//!
//! # Architecture
//!
//! State (`ExplorerState`) is registered in `PluginStateRegistry` and accessed
//! via `RuntimeContext::with_state_mut::<ExplorerState>()`.
//!
//! Commands emit `EventBus` events that are handled by event subscriptions.

use std::any::TypeId;

use reovim_core::{
    display::{DisplayInfo, EditModeKey},
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    ui_component::ComponentId,
};

mod command;
mod events;
mod node;
mod state;
mod tree;

// Plugin command types
use command::{
    ExplorerCancelInputCommand, ExplorerClearFilterCommand, ExplorerCloseCommand,
    ExplorerCloseParentCommand, ExplorerConfirmInputCommand, ExplorerCreateDirCommand,
    ExplorerCreateFileCommand, ExplorerCursorDownCommand, ExplorerCursorUpCommand,
    ExplorerCutCommand, ExplorerDeleteCommand, ExplorerExitVisualCommand, ExplorerFilterCommand,
    ExplorerFocusEditorCommand, ExplorerGoToParentCommand, ExplorerGotoFirstCommand,
    ExplorerGotoLastCommand, ExplorerInputBackspaceCommand, ExplorerOpenNodeCommand,
    ExplorerPageDownCommand, ExplorerPageUpCommand, ExplorerPasteCommand, ExplorerRefreshCommand,
    ExplorerRenameCommand, ExplorerSelectAllCommand, ExplorerToggleHiddenCommand,
    ExplorerToggleNodeCommand, ExplorerToggleSelectCommand, ExplorerToggleSizesCommand,
    ExplorerVisualModeCommand, ExplorerYankCommand, ToggleExplorerCommand,
};

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
        // CorePlugin dependency
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        self.register_display_info(ctx);
        self.register_navigation_commands(ctx);
        self.register_tree_commands(ctx);
        self.register_file_commands(ctx);
        self.register_clipboard_commands(ctx);
        self.register_visual_commands(ctx);
        self.register_input_commands(ctx);
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Initialize ExplorerState with current working directory
        // If cwd fails, explorer will show error on first use
        if let Ok(cwd) = std::env::current_dir()
            && let Ok(state) = ExplorerState::new(cwd)
        {
            registry.register(state);
        }
    }
}

/// Component ID for the explorer
pub const COMPONENT_ID: ComponentId = ComponentId("explorer");

#[allow(clippy::unused_self)]
impl ExplorerPlugin {
    fn register_display_info(&self, ctx: &mut PluginContext) {
        ctx.register_display(COMPONENT_ID, DisplayInfo::new(" EXPLORER ", "󰙅 "));
        ctx.register_component_mode_display(
            COMPONENT_ID,
            EditModeKey::Insert,
            DisplayInfo::new(" EXPLORER | INSERT ", "󰙅 "),
        );
        ctx.register_component_mode_display(
            COMPONENT_ID,
            EditModeKey::VisualChar,
            DisplayInfo::new(" EXPLORER | VISUAL ", "󰙅 "),
        );
    }

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
        let _ = ctx.register_command(ToggleExplorerCommand);
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

// Re-export key types for external use
pub use {
    events::{
        ExplorerCancelInputEvent, ExplorerClearFilterEvent, ExplorerCloseEvent,
        ExplorerCloseParentEvent, ExplorerConfirmInputEvent, ExplorerCreateDirEvent,
        ExplorerCreateFileEvent, ExplorerCursorDownEvent, ExplorerCursorUpEvent, ExplorerCutEvent,
        ExplorerDeleteEvent, ExplorerExitVisualEvent, ExplorerFocusEditorEvent,
        ExplorerGoToParentEvent, ExplorerGotoFirstEvent, ExplorerGotoLastEvent,
        ExplorerInputBackspaceEvent, ExplorerInputCharEvent, ExplorerOpenNodeEvent,
        ExplorerPageDownEvent, ExplorerPageUpEvent, ExplorerPasteEvent, ExplorerRefreshEvent,
        ExplorerRenameEvent, ExplorerSelectAllEvent, ExplorerStartFilterEvent, ExplorerToggleEvent,
        ExplorerToggleHiddenEvent, ExplorerToggleNodeEvent, ExplorerToggleSelectEvent,
        ExplorerToggleSizesEvent, ExplorerVisualModeEvent, ExplorerYankEvent,
    },
    node::FileNode,
    state::ExplorerState,
    tree::FileTree,
};
