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

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    bind::{CommandRef, EditModeKind, KeymapScope},
    display::{DisplayInfo, EditModeKey},
    event_bus::{EventBus, EventResult},
    keys,
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    ui_component::ComponentId,
};

mod command;
mod events;
mod node;
mod provider;
mod render;
mod state;
mod tree;
mod window;

#[cfg(test)]
mod tests;

// Re-export state, events, and provider
pub use provider::ExplorerBufferProvider;
pub use state::ExplorerState;

/// Command IDs for explorer commands
pub mod command_id {
    use reovim_core::command::id::CommandId;

    pub const TOGGLE_EXPLORER: CommandId = CommandId::new("toggle_explorer");

    // Navigation
    pub const CURSOR_UP: CommandId = CommandId::new("explorer_cursor_up");
    pub const CURSOR_DOWN: CommandId = CommandId::new("explorer_cursor_down");
    pub const PAGE_UP: CommandId = CommandId::new("explorer_page_up");
    pub const PAGE_DOWN: CommandId = CommandId::new("explorer_page_down");
    pub const GOTO_FIRST: CommandId = CommandId::new("explorer_goto_first");
    pub const GOTO_LAST: CommandId = CommandId::new("explorer_goto_last");
    pub const GO_TO_PARENT: CommandId = CommandId::new("explorer_go_to_parent");

    // Tree operations
    pub const TOGGLE_NODE: CommandId = CommandId::new("explorer_toggle_node");
    pub const OPEN_NODE: CommandId = CommandId::new("explorer_open_node");
    pub const CLOSE_PARENT: CommandId = CommandId::new("explorer_close_parent");
    pub const REFRESH: CommandId = CommandId::new("explorer_refresh");
    pub const TOGGLE_HIDDEN: CommandId = CommandId::new("explorer_toggle_hidden");
    pub const TOGGLE_SIZES: CommandId = CommandId::new("explorer_toggle_sizes");
    pub const CLOSE: CommandId = CommandId::new("explorer_close");
    pub const FOCUS_EDITOR: CommandId = CommandId::new("explorer_focus_editor");

    // File operations
    pub const CREATE_FILE: CommandId = CommandId::new("explorer_create_file");
    pub const CREATE_DIR: CommandId = CommandId::new("explorer_create_dir");
    pub const RENAME: CommandId = CommandId::new("explorer_rename");
    pub const DELETE: CommandId = CommandId::new("explorer_delete");
    pub const FILTER: CommandId = CommandId::new("explorer_filter");
    pub const CLEAR_FILTER: CommandId = CommandId::new("explorer_clear_filter");

    // Clipboard
    pub const YANK: CommandId = CommandId::new("explorer_yank");
    pub const CUT: CommandId = CommandId::new("explorer_cut");
    pub const PASTE: CommandId = CommandId::new("explorer_paste");

    // Visual mode
    pub const VISUAL_MODE: CommandId = CommandId::new("explorer_visual_mode");
    pub const TOGGLE_SELECT: CommandId = CommandId::new("explorer_toggle_select");
    pub const SELECT_ALL: CommandId = CommandId::new("explorer_select_all");
    pub const EXIT_VISUAL: CommandId = CommandId::new("explorer_exit_visual");

    // Input mode
    pub const CONFIRM_INPUT: CommandId = CommandId::new("explorer_confirm_input");
    pub const CANCEL_INPUT: CommandId = CommandId::new("explorer_cancel_input");
    pub const INPUT_BACKSPACE: CommandId = CommandId::new("explorer_input_backspace");
}

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
        self.register_keybindings(ctx);
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Initialize ExplorerState with current working directory
        // If cwd fails, explorer will show error on first use
        if let Ok(cwd) = std::env::current_dir()
            && let Ok(state) = ExplorerState::new(cwd)
        {
            registry.register(state);
            tracing::info!("ExplorerPlugin: registered state");
        } else {
            tracing::error!("ExplorerPlugin: failed to create ExplorerState");
        }

        // Register window provider so the explorer can create its window
        registry.register_window_provider(Arc::new(window::ExplorerWindowProvider));
        tracing::info!("ExplorerPlugin: registered window provider");
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        use events::*;
        use reovim_core::{
            event_bus::core_events::{RequestFocusChange, RequestOpenFile},
            ui_component::ComponentId,
        };

        // Navigation events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCursorUpEvent, _>(100, move |event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_cursor(-(event.count as isize));
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCursorDownEvent, _>(100, move |event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_cursor(event.count as isize);
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerPageUpEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_page(20, false); // TODO: get actual height
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerPageDownEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_page(20, true); // TODO: get actual height
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerGotoFirstEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_to_first();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerGotoLastEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_to_last();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerGoToParentEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.go_to_parent();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Open file or toggle directory
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerOpenNodeEvent, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerOpenNodeEvent received");

            let result = state_clone.with::<ExplorerState, _, _>(|explorer| {
                if let Some(node) = explorer.current_node() {
                    if node.is_file() {
                        // Open the file
                        Some((node.path.clone(), true))
                    } else if node.is_dir() {
                        // Toggle directory
                        Some((node.path.clone(), false))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

            if let Some(Some((path, is_file))) = result {
                if is_file {
                    tracing::info!("ExplorerPlugin: Requesting to open file: {:?}", path);
                    ctx.emit(RequestOpenFile { path });
                    // Return focus to editor after opening file
                    ctx.emit(RequestFocusChange {
                        target: ComponentId::EDITOR,
                    });
                } else {
                    tracing::info!("ExplorerPlugin: Toggling directory: {:?}", path);
                    state_clone.with_mut::<ExplorerState, _, _>(|explorer| {
                        let _ = explorer.toggle_current();
                    });
                }
                ctx.request_render();
            }

            EventResult::Handled
        });

        // Tree manipulation events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleNodeEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                let _ = s.toggle_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCloseParentEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.collapse_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerRefreshEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                let _ = s.refresh();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleHiddenEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.toggle_hidden();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleSizesEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.toggle_sizes();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Clipboard events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerYankEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.yank_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCutEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.cut_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerPasteEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                let _ = s.paste();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // File operation events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCreateFileEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_create_file();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCreateDirEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_create_dir();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerRenameEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_rename();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerDeleteEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_delete();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerStartFilterEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_filter();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerClearFilterEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.clear_filter();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Input events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerConfirmInputEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                let _ = s.confirm_input();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCancelInputEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.cancel_input();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerInputCharEvent, _>(100, move |event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.input_char(event.c);
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerInputBackspaceEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.input_backspace();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Visual selection events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerVisualModeEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.enter_visual_mode();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleSelectEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.toggle_select_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerSelectAllEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.select_all();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerExitVisualEvent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.exit_visual_mode();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Toggle explorer visibility
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleEvent, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerToggleEvent received");

            let old_visible = state_clone
                .with::<ExplorerState, _, _>(|e| e.visible)
                .unwrap_or(false);

            state_clone.with_mut::<ExplorerState, _, _>(|explorer| {
                explorer.toggle_visibility();
            });

            let new_visible = state_clone
                .with::<ExplorerState, _, _>(|e| e.visible)
                .unwrap_or(false);

            tracing::info!("ExplorerPlugin: Explorer toggled: {} -> {}", old_visible, new_visible);

            // Change focus based on visibility
            if new_visible {
                // Explorer is now visible - give it focus
                ctx.emit(RequestFocusChange {
                    target: COMPONENT_ID,
                });
                tracing::info!("ExplorerPlugin: Requesting focus change to explorer");
            } else {
                // Explorer is now hidden - return focus to editor
                ctx.emit(RequestFocusChange {
                    target: ComponentId::EDITOR,
                });
                tracing::info!("ExplorerPlugin: Requesting focus change to editor");
            }

            ctx.request_render();
            tracing::info!("ExplorerPlugin: Render requested");
            EventResult::Handled
        });

        // Close explorer and return focus to editor
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCloseEvent, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerCloseEvent received");

            state_clone.with_mut::<ExplorerState, _, _>(|explorer| {
                explorer.visible = false;
            });

            ctx.emit(RequestFocusChange {
                target: ComponentId::EDITOR,
            });
            tracing::info!("ExplorerPlugin: Requesting focus change to editor");

            ctx.request_render();
            EventResult::Handled
        });

        // Focus editor (without closing explorer)
        let _state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerFocusEditorEvent, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerFocusEditorEvent received");

            ctx.emit(RequestFocusChange {
                target: ComponentId::EDITOR,
            });
            tracing::info!("ExplorerPlugin: Requesting focus change to editor");

            ctx.request_render();
            EventResult::Handled
        });
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

    fn register_keybindings(&self, ctx: &mut PluginContext) {
        let editor_normal = KeymapScope::editor_normal();
        let explorer_normal = KeymapScope::Component {
            id: COMPONENT_ID,
            mode: EditModeKind::Normal,
        };

        // Global keybinding: Space+e to toggle explorer
        ctx.bind_key_scoped(
            editor_normal,
            keys![Space 'e'],
            CommandRef::Registered(command_id::TOGGLE_EXPLORER),
        );

        // Explorer-specific keybindings (when explorer is focused)

        // Navigation
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['j'],
            CommandRef::Registered(command_id::CURSOR_DOWN),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['k'],
            CommandRef::Registered(command_id::CURSOR_UP),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys![(Ctrl 'd')],
            CommandRef::Registered(command_id::PAGE_DOWN),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys![(Ctrl 'u')],
            CommandRef::Registered(command_id::PAGE_UP),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['g' 'g'],
            CommandRef::Registered(command_id::GOTO_FIRST),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['G'],
            CommandRef::Registered(command_id::GOTO_LAST),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['-'],
            CommandRef::Registered(command_id::GO_TO_PARENT),
        );

        // Tree operations
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys![Enter],
            CommandRef::Registered(command_id::OPEN_NODE),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['l'],
            CommandRef::Registered(command_id::OPEN_NODE),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['h'],
            CommandRef::Registered(command_id::CLOSE_PARENT),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys![Space],
            CommandRef::Registered(command_id::TOGGLE_NODE),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['R'],
            CommandRef::Registered(command_id::REFRESH),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['H'],
            CommandRef::Registered(command_id::TOGGLE_HIDDEN),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['s'],
            CommandRef::Registered(command_id::TOGGLE_SIZES),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['q'],
            CommandRef::Registered(command_id::CLOSE),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::FOCUS_EDITOR),
        );

        // File operations
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['a'],
            CommandRef::Registered(command_id::CREATE_FILE),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['A'],
            CommandRef::Registered(command_id::CREATE_DIR),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['r'],
            CommandRef::Registered(command_id::RENAME),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['d'],
            CommandRef::Registered(command_id::DELETE),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['/'],
            CommandRef::Registered(command_id::FILTER),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['c'],
            CommandRef::Registered(command_id::CLEAR_FILTER),
        );

        // Clipboard
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['y'],
            CommandRef::Registered(command_id::YANK),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['x'],
            CommandRef::Registered(command_id::CUT),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['p'],
            CommandRef::Registered(command_id::PASTE),
        );

        // Visual mode
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['v'],
            CommandRef::Registered(command_id::VISUAL_MODE),
        );

        tracing::info!("ExplorerPlugin: registered keybinding Space+e and explorer navigation");
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
        ExplorerToggleHiddenEvent,
        ExplorerToggleNodeEvent, ExplorerToggleSelectEvent, ExplorerToggleSizesEvent,
        ExplorerVisualModeEvent, ExplorerYankEvent,
    },
    node::FileNode,
    tree::FileTree,
};
