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
    event_bus::{EventBus, EventResult},
    keys,
    modd::{ComponentId, EditMode, InsertVariant, VisualVariant},
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

mod command;
mod node;
mod provider;
mod render;
mod state;
mod tree;
mod window;

#[cfg(test)]
mod tests;

// Re-export state, events, and provider
pub use {provider::ExplorerBufferProvider, state::ExplorerState};

/// Command IDs for explorer commands
pub mod command_id {
    use reovim_core::command::id::CommandId;

    pub const TOGGLE_EXPLORER: CommandId = CommandId::new("explorer_toggle");

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

    // File info popup
    pub const SHOW_INFO: CommandId = CommandId::new("explorer_show_info");
    pub const CLOSE_POPUP: CommandId = CommandId::new("explorer_close_popup");
    pub const COPY_PATH: CommandId = CommandId::new("explorer_copy_path");
}

// Plugin unified command-event types
pub use command::{
    ExplorerCancelInput, ExplorerClearFilter, ExplorerClose, ExplorerCloseParent,
    ExplorerClosePopup, ExplorerConfirmInput, ExplorerCopyPath, ExplorerCreateDir,
    ExplorerCreateFile, ExplorerCursorDown, ExplorerCursorUp, ExplorerCut, ExplorerDelete,
    ExplorerExitVisual, ExplorerFocusEditor, ExplorerGoToParent, ExplorerGotoFirst,
    ExplorerGotoLast, ExplorerInputBackspace, ExplorerInputChar, ExplorerOpenNode,
    ExplorerPageDown, ExplorerPageUp, ExplorerPaste, ExplorerRefresh, ExplorerRename,
    ExplorerSelectAll, ExplorerShowInfo, ExplorerStartFilter, ExplorerToggle, ExplorerToggleHidden,
    ExplorerToggleNode, ExplorerToggleSelect, ExplorerToggleSizes, ExplorerVisualMode,
    ExplorerYank,
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
        // Input handling is done via PluginTextInput/PluginBackspace event subscriptions
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

        // Register plugin windows
        registry.register_plugin_window(Arc::new(window::ExplorerPluginWindow));
        registry.register_plugin_window(Arc::new(window::FileDetailsPluginWindow));
        tracing::info!("ExplorerPlugin: registered plugin windows");
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        use reovim_core::{
            event_bus::core_events::{
                PluginBackspace, PluginTextInput, RequestFocusChange, RequestModeChange,
                RequestOpenFile,
            },
            modd::{EditMode, ModeState, SubMode},
        };

        // Handle text input from runtime (PluginTextInput event)
        let state_clone = Arc::clone(&state);
        bus.subscribe::<PluginTextInput, _>(100, move |event, ctx| {
            // Only handle if we're the target
            if event.target != COMPONENT_ID {
                return EventResult::NotHandled;
            }

            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.input_char(event.c);
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Handle backspace from runtime (PluginBackspace event)
        let state_clone = Arc::clone(&state);
        bus.subscribe::<PluginBackspace, _>(100, move |event, ctx| {
            // Only handle if we're the target
            if event.target != COMPONENT_ID {
                return EventResult::NotHandled;
            }

            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.input_backspace();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Navigation events (sync popup with cursor if visible)
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCursorUp, _>(100, move |event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_cursor(-(event.count as isize));
                s.update_scroll();
                s.sync_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCursorDown, _>(100, move |event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_cursor(event.count as isize);
                s.update_scroll();
                s.sync_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerPageUp, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_page(s.visible_height, false);
                s.update_scroll();
                s.sync_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerPageDown, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_page(s.visible_height, true);
                s.update_scroll();
                s.sync_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerGotoFirst, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_to_first();
                s.update_scroll();
                s.sync_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerGotoLast, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.move_to_last();
                s.update_scroll();
                s.sync_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerGoToParent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.go_to_parent();
                s.update_scroll();
                s.sync_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Open file or toggle directory
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerOpenNode, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerOpenNode received");

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
        bus.subscribe::<ExplorerToggleNode, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                let _ = s.toggle_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCloseParent, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.collapse_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerRefresh, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                let _ = s.refresh();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleHidden, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.toggle_hidden();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleSizes, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.toggle_sizes();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Clipboard events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerYank, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.yank_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCut, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.cut_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerPaste, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                let _ = s.paste();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // File operation events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCreateFile, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_create_file();
            });

            // Enter Interactor sub-mode so characters route to focus handler
            let mode = ModeState::with_interactor_id_sub_mode(
                COMPONENT_ID,
                EditMode::Normal,
                SubMode::Interactor(COMPONENT_ID),
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCreateDir, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_create_dir();
            });

            // Enter Interactor sub-mode so characters route to focus handler
            let mode = ModeState::with_interactor_id_sub_mode(
                COMPONENT_ID,
                EditMode::Normal,
                SubMode::Interactor(COMPONENT_ID),
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerRename, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_rename();
            });

            // Enter Interactor sub-mode so characters route to focus handler
            let mode = ModeState::with_interactor_id_sub_mode(
                COMPONENT_ID,
                EditMode::Normal,
                SubMode::Interactor(COMPONENT_ID),
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerDelete, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_delete();
            });

            // Enter Interactor sub-mode for confirmation input
            let mode = ModeState::with_interactor_id_sub_mode(
                COMPONENT_ID,
                EditMode::Normal,
                SubMode::Interactor(COMPONENT_ID),
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerStartFilter, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.start_filter();
            });

            // Enter Interactor sub-mode so characters route to focus handler
            let mode = ModeState::with_interactor_id_sub_mode(
                COMPONENT_ID,
                EditMode::Normal,
                SubMode::Interactor(COMPONENT_ID),
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerClearFilter, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.clear_filter();
            });

            // Exit Interactor sub-mode back to normal explorer mode
            let mode = ModeState::with_interactor_id_and_mode(COMPONENT_ID, EditMode::Normal);
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        // Input events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerConfirmInput, _>(100, move |_event, ctx| {
            // Check if popup is visible
            let popup_visible = state_clone
                .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
                .unwrap_or(false);

            if popup_visible {
                // Close the popup
                state_clone.with_mut::<ExplorerState, _, _>(|s| {
                    s.close_popup();
                });
                ctx.request_render();
                return EventResult::Handled;
            }

            // Check if in input mode
            let in_input_mode = state_clone
                .with::<ExplorerState, _, _>(|s| {
                    !matches!(s.input_mode, crate::state::ExplorerInputMode::None)
                })
                .unwrap_or(false);

            if in_input_mode {
                // Confirm the input (create file, rename, etc.)
                state_clone.with_mut::<ExplorerState, _, _>(|s| {
                    let _ = s.confirm_input();
                });

                // Exit Interactor sub-mode back to normal explorer mode
                let mode = ModeState::with_interactor_id_and_mode(COMPONENT_ID, EditMode::Normal);
                ctx.emit(RequestModeChange { mode });
            } else {
                // Not in input mode - open the selected node (file/directory)
                ctx.emit(ExplorerOpenNode);
            }

            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCancelInput, _>(100, move |_event, ctx| {
            // Check if popup is visible
            let popup_visible = state_clone
                .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
                .unwrap_or(false);

            if popup_visible {
                // Close the popup
                state_clone.with_mut::<ExplorerState, _, _>(|s| {
                    s.close_popup();
                });
                ctx.request_render();
                return EventResult::Handled;
            }

            // Check if in input mode
            let in_input_mode = state_clone
                .with::<ExplorerState, _, _>(|s| {
                    !matches!(s.input_mode, crate::state::ExplorerInputMode::None)
                })
                .unwrap_or(false);

            if in_input_mode {
                // Cancel the input
                state_clone.with_mut::<ExplorerState, _, _>(|s| {
                    s.cancel_input();
                });

                // Exit Interactor sub-mode back to normal explorer mode
                let mode = ModeState::with_interactor_id_and_mode(COMPONENT_ID, EditMode::Normal);
                ctx.emit(RequestModeChange { mode });
            } else {
                // Not in input mode - return focus to editor
                ctx.emit(ExplorerFocusEditor);
            }

            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerInputChar, _>(100, move |event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.input_char(event.c);
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerInputBackspace, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.input_backspace();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Visual selection events
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerVisualMode, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.enter_visual_mode();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggleSelect, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.toggle_select_current();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerSelectAll, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.select_all();
            });
            ctx.request_render();
            EventResult::Handled
        });

        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerExitVisual, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.exit_visual_mode();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Toggle explorer visibility
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerToggle, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerToggle received");

            let old_visible = state_clone
                .with::<ExplorerState, _, _>(|e| e.visible)
                .unwrap_or(false);

            state_clone.with_mut::<ExplorerState, _, _>(|explorer| {
                explorer.toggle_visibility();
            });

            let (new_visible, width) = state_clone
                .with::<ExplorerState, _, _>(|e| (e.visible, e.width))
                .unwrap_or((false, 0));

            tracing::info!("ExplorerPlugin: Explorer toggled: {} -> {}", old_visible, new_visible);

            // Update left panel width for blocking layout
            if new_visible {
                state_clone.set_left_panel_width(width);
            } else {
                state_clone.set_left_panel_width(0);
            }

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
        bus.subscribe::<ExplorerClose, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerClose received");

            state_clone.with_mut::<ExplorerState, _, _>(|explorer| {
                explorer.visible = false;
            });

            // Clear left panel width since explorer is hidden
            state_clone.set_left_panel_width(0);

            ctx.emit(RequestFocusChange {
                target: ComponentId::EDITOR,
            });
            tracing::info!("ExplorerPlugin: Requesting focus change to editor");

            ctx.request_render();
            EventResult::Handled
        });

        // Focus editor (without closing explorer)
        let _state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerFocusEditor, _>(100, move |_event, ctx| {
            tracing::info!("ExplorerPlugin: ExplorerFocusEditor received");

            ctx.emit(RequestFocusChange {
                target: ComponentId::EDITOR,
            });
            tracing::info!("ExplorerPlugin: Requesting focus change to editor");

            ctx.request_render();
            EventResult::Handled
        });

        // Show file details popup
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerShowInfo, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.show_file_details();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Close file details popup
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerClosePopup, _>(100, move |_event, ctx| {
            state_clone.with_mut::<ExplorerState, _, _>(|s| {
                s.close_popup();
            });
            ctx.request_render();
            EventResult::Handled
        });

        // Copy path to clipboard
        let state_clone = Arc::clone(&state);
        bus.subscribe::<ExplorerCopyPath, _>(100, move |_event, ctx| {
            use reovim_core::event_bus::core_events::RequestSetRegister;

            let path = state_clone
                .with::<ExplorerState, _, _>(|s| {
                    s.current_node()
                        .map(|n| n.path.to_string_lossy().to_string())
                })
                .flatten();

            if let Some(path) = path {
                // Set both unnamed register and system clipboard
                ctx.emit(RequestSetRegister {
                    register: None, // Unnamed register for 'p' paste
                    text: path.clone(),
                });
                ctx.emit(RequestSetRegister {
                    register: Some('+'), // System clipboard
                    text: path,
                });

                state_clone.with_mut::<ExplorerState, _, _>(|s| {
                    s.close_popup();
                    s.message = Some("Path copied to clipboard".to_string());
                });
            }

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
        ctx.display_info(COMPONENT_ID)
            .default(" EXPLORER ", "󰙅 ")
            .when_mode(&EditMode::Insert(InsertVariant::Standard), " EXPLORER | INSERT ", "󰙅 ")
            .when_mode(&EditMode::Visual(VisualVariant::Char), " EXPLORER | VISUAL ", "󰙅 ")
            .register();
    }

    fn register_navigation_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerCursorUp::new(1));
        let _ = ctx.register_command(ExplorerCursorDown::new(1));
        let _ = ctx.register_command(ExplorerPageUp);
        let _ = ctx.register_command(ExplorerPageDown);
        let _ = ctx.register_command(ExplorerGotoFirst);
        let _ = ctx.register_command(ExplorerGotoLast);
        let _ = ctx.register_command(ExplorerGoToParent);
    }

    fn register_tree_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerToggle);
        let _ = ctx.register_command(ExplorerToggleNode);
        let _ = ctx.register_command(ExplorerOpenNode);
        let _ = ctx.register_command(ExplorerCloseParent);
        let _ = ctx.register_command(ExplorerRefresh);
        let _ = ctx.register_command(ExplorerToggleHidden);
        let _ = ctx.register_command(ExplorerToggleSizes);
        let _ = ctx.register_command(ExplorerClose);
        let _ = ctx.register_command(ExplorerFocusEditor);
        let _ = ctx.register_command(ExplorerShowInfo);
        let _ = ctx.register_command(ExplorerClosePopup);
        let _ = ctx.register_command(ExplorerCopyPath);
    }

    fn register_file_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerCreateFile);
        let _ = ctx.register_command(ExplorerCreateDir);
        let _ = ctx.register_command(ExplorerRename);
        let _ = ctx.register_command(ExplorerDelete);
        let _ = ctx.register_command(ExplorerStartFilter);
        let _ = ctx.register_command(ExplorerClearFilter);
    }

    fn register_clipboard_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerYank);
        let _ = ctx.register_command(ExplorerCut);
        let _ = ctx.register_command(ExplorerPaste);
    }

    fn register_visual_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerVisualMode);
        let _ = ctx.register_command(ExplorerToggleSelect);
        let _ = ctx.register_command(ExplorerSelectAll);
        let _ = ctx.register_command(ExplorerExitVisual);
    }

    fn register_input_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(ExplorerConfirmInput);
        let _ = ctx.register_command(ExplorerCancelInput);
        let _ = ctx.register_command(ExplorerInputBackspace);
    }

    fn register_keybindings(&self, ctx: &mut PluginContext) {
        use reovim_core::bind::SubModeKind;

        let editor_normal = KeymapScope::editor_normal();
        let explorer_normal = KeymapScope::Component {
            id: COMPONENT_ID,
            mode: EditModeKind::Normal,
        };
        let explorer_interactor = KeymapScope::SubMode(SubModeKind::Interactor(COMPONENT_ID));

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

        // Tree operations / Input confirmation
        // Enter: Confirm input if in input mode, otherwise open node
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys![Enter],
            CommandRef::Registered(command_id::CONFIRM_INPUT),
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
            CommandRef::Registered(command_id::SHOW_INFO),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['t'],
            CommandRef::Registered(command_id::COPY_PATH),
        );
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys!['q'],
            CommandRef::Registered(command_id::CLOSE),
        );
        // Escape: Cancel input if in input mode, otherwise focus editor
        ctx.bind_key_scoped(
            explorer_normal.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::CANCEL_INPUT),
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

        // Interactor sub-mode keybindings (for input mode: CreateFile, Rename, etc.)
        // Enter: Confirm input
        ctx.bind_key_scoped(
            explorer_interactor.clone(),
            keys![Enter],
            CommandRef::Registered(command_id::CONFIRM_INPUT),
        );
        // Escape: Cancel input
        ctx.bind_key_scoped(
            explorer_interactor.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::CANCEL_INPUT),
        );
        // Backspace: Delete character from input
        ctx.bind_key_scoped(
            explorer_interactor,
            keys![Backspace],
            CommandRef::Registered(command_id::INPUT_BACKSPACE),
        );

        tracing::info!(
            "ExplorerPlugin: registered keybinding Space+e, explorer navigation, and interactor input"
        );
    }
}
