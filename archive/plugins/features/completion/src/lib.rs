//! Code completion plugin for reovim
//!
//! This plugin provides auto-completion functionality:
//! - Trigger completion popup
//! - Navigate suggestions with next/prev
//! - Confirm selection or dismiss
//!
//! # Architecture
//!
//! This plugin follows the treesitter decoupling pattern:
//! - Defines `SourceSupport` trait for external sources to implement
//! - Uses background saturator for non-blocking completion
//! - Uses ArcSwap cache for lock-free render access
//! - Communicates via `EventBus` events

mod cache;
mod commands;
mod events;
mod registry;
mod saturator;
mod source;
mod state;
mod window;

use std::{any::TypeId, sync::Arc};

// Re-export public API
pub use {
    cache::{CompletionCache, CompletionSnapshot},
    commands::{
        CompletionConfirm, CompletionDismiss, CompletionSelectNext, CompletionSelectPrev,
        CompletionTrigger, CompletionTriggered,
    },
    events::{CompletionDismissed, CompletionReady, RegisterSource},
    registry::{SourceRegistry, SourceSupport},
    saturator::{CompletionRequest, CompletionSaturatorHandle, spawn_completion_saturator},
    source::BufferWordsSource,
    state::SharedCompletionManager,
    window::CompletionPluginWindow,
};

// Re-export from core for convenience
pub use reovim_core::completion::{CompletionContext, CompletionItem, CompletionKind};

use reovim_core::{
    bind::CommandRef,
    command::{CommandContext, id::CommandId},
    event::CommandEvent,
    event_bus::{
        EventBus, EventResult,
        core_events::{BufferModification, BufferModified, RequestInsertText},
    },
    keys,
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

/// Plugin-local command IDs
pub mod command_id {
    use super::CommandId;

    pub const COMPLETION_TRIGGER: CommandId = CommandId::new("completion_trigger");
    pub const COMPLETION_NEXT: CommandId = CommandId::new("completion_next");
    pub const COMPLETION_PREV: CommandId = CommandId::new("completion_prev");
    pub const COMPLETION_CONFIRM: CommandId = CommandId::new("completion_confirm");
    pub const COMPLETION_DISMISS: CommandId = CommandId::new("completion_dismiss");
}

/// Code completion plugin
///
/// Provides auto-completion with:
/// - Background saturator for non-blocking computation
/// - Lock-free cache for responsive UI
/// - Extensible source system via `SourceSupport` trait
pub struct CompletionPlugin {
    manager: Arc<SharedCompletionManager>,
}

impl CompletionPlugin {
    /// Create a new completion plugin
    #[must_use]
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SharedCompletionManager::new()),
        }
    }
}

impl Default for CompletionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CompletionPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:completion")
    }

    fn name(&self) -> &'static str {
        "Completion"
    }

    fn description(&self) -> &'static str {
        "Auto-completion with background processing"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register commands (unified types)
        let _ = ctx.register_command(CompletionTrigger::new(0));
        let _ = ctx.register_command(CompletionSelectNext);
        let _ = ctx.register_command(CompletionSelectPrev);
        let _ = ctx.register_command(CompletionConfirm);
        let _ = ctx.register_command(CompletionDismiss);

        // Register keybindings
        use reovim_core::bind::KeymapScope;
        let insert_mode = KeymapScope::editor_insert();

        // Alt-Space to trigger completion in insert mode
        ctx.bind_key_scoped(
            insert_mode.clone(),
            keys![(Alt Space)],
            CommandRef::Registered(command_id::COMPLETION_TRIGGER),
        );

        // Navigation keybindings when completion is active
        ctx.bind_key_scoped(
            insert_mode.clone(),
            keys![(Ctrl 'n')],
            CommandRef::Registered(command_id::COMPLETION_NEXT),
        );

        ctx.bind_key_scoped(
            insert_mode.clone(),
            keys![(Ctrl 'p')],
            CommandRef::Registered(command_id::COMPLETION_PREV),
        );

        // Ctrl+y to confirm completion selection (vim convention)
        ctx.bind_key_scoped(
            insert_mode.clone(),
            keys![(Ctrl 'y')],
            CommandRef::Registered(command_id::COMPLETION_CONFIRM),
        );

        // Note: Tab for confirm is NOT supported due to architectural limitations.
        // When a keybinding is found, the Tab fallback (insert '\t') is skipped.
        // The keybinding system doesn't support "fallback" when a command returns NotHandled.
        // See Issue #4 in docs/cmp.md for details.
        // Users should use Ctrl+y to confirm completion.
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the shared manager for cross-plugin access
        registry.register(Arc::clone(&self.manager));

        // Register the plugin window
        registry.register_plugin_window(Arc::new(CompletionPluginWindow::new(Arc::clone(
            &self.manager,
        ))));
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Capture tokio runtime handle for use in EventBus handlers
        // (EventBus handlers run on std::thread, not tokio runtime)
        let rt_handle = tokio::runtime::Handle::current();

        // Subscribe to CompletionTriggered events (from CompletionTrigger command)
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<CompletionTriggered, _>(100, move |event, ctx| {
            // If completion is already active, check if cursor moved outside valid range
            if manager.is_active() {
                let snapshot = manager.snapshot();
                let req = &event.request;

                // Dismiss if cursor moved to different line
                if req.cursor_row != snapshot.cursor_row {
                    tracing::debug!("Completion dismissed: line changed");
                    manager.dismiss();
                    ctx.request_render();
                    return EventResult::Handled;
                }

                // Dismiss if cursor moved before word start
                if req.cursor_col < snapshot.word_start_col {
                    tracing::debug!("Completion dismissed: cursor before word start");
                    manager.dismiss();
                    ctx.request_render();
                    return EventResult::Handled;
                }

                // Dismiss if prefix is empty (deleted all characters)
                if req.prefix.is_empty() {
                    tracing::debug!("Completion dismissed: empty prefix");
                    manager.dismiss();
                    ctx.request_render();
                    return EventResult::Handled;
                }
            }

            tracing::info!("CompletionTriggered event received, prefix={}", event.request.prefix);
            manager.request_completion(event.request.clone());
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to RegisterSource events from external plugins
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<RegisterSource, _>(100, move |event, _ctx| {
            manager.register_source(Arc::clone(&event.source));
            EventResult::Handled
        });

        // Subscribe to CompletionSelectNext command
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<CompletionSelectNext, _>(100, move |_event, ctx| {
            if manager.is_active() {
                manager.select_next();
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Subscribe to CompletionSelectPrev command
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<CompletionSelectPrev, _>(100, move |_event, ctx| {
            if manager.is_active() {
                manager.select_prev();
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Subscribe to CompletionDismiss command
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<CompletionDismiss, _>(100, move |_event, ctx| {
            if manager.is_active() {
                manager.dismiss();
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Subscribe to CompletionConfirm command
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<CompletionConfirm, _>(100, move |_event, ctx| {
            if !manager.is_active() {
                return EventResult::NotHandled;
            }

            let snapshot = manager.snapshot();
            let Some(item) = snapshot.selected_item() else {
                return EventResult::NotHandled;
            };

            // Delete the typed prefix and insert the full completion text
            // This replaces "pkg" with "CARGO_PKG" instead of appending
            let prefix_len = snapshot.prefix.len();
            let insert_text = item.insert_text.clone();

            if !insert_text.is_empty() || prefix_len > 0 {
                ctx.emit(RequestInsertText {
                    text: insert_text,
                    move_cursor_left: false,
                    delete_prefix_len: prefix_len,
                });
            }

            manager.dismiss();
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to ModeChanged to dismiss completion when leaving insert
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<reovim_core::event_bus::core_events::ModeChanged, _>(
            100,
            move |event, ctx| {
                // Dismiss if leaving insert mode
                if !event.to.contains("Insert") && manager.is_active() {
                    manager.dismiss();
                    ctx.request_render();
                }
                EventResult::Handled
            },
        );

        // Cursor position tracking is handled via CursorMoved events (emitted by runtime).
        // Additional checks are in CompletionTriggered handler for completion re-triggers.

        // Subscribe to BufferModified for auto-popup and live update
        let manager = Arc::clone(&self.manager);
        let rt_handle = rt_handle.clone();
        bus.subscribe::<BufferModified, _>(100, move |event, _ctx| {
            let is_active = manager.is_active();

            match &event.modification {
                BufferModification::Insert { text, .. } => {
                    // Ignore whitespace-only insertions for auto-popup
                    if text.chars().all(|c| c.is_whitespace()) {
                        if is_active {
                            // Dismiss on whitespace (space/enter ends word)
                            manager.dismiss();
                        }
                        return EventResult::NotHandled;
                    }
                }
                BufferModification::Delete { .. } => {
                    // On delete (backspace), re-trigger if active to update filter
                    if !is_active {
                        return EventResult::NotHandled;
                    }
                }
                BufferModification::Replace { .. } | BufferModification::FullReplace => {
                    // On replace operations, dismiss completion
                    if is_active {
                        manager.dismiss();
                    }
                    return EventResult::NotHandled;
                }
            }

            let generation = manager.next_debounce_generation();
            let manager = Arc::clone(&manager);
            let buffer_id = event.buffer_id;

            // Spawn task to trigger/update completion
            // Use captured rt_handle since EventBus handlers run on std::thread (#120)
            rt_handle.spawn(async move {
                if !is_active {
                    // Completion not active: use debounce delay for auto-popup
                    tokio::time::sleep(std::time::Duration::from_millis(
                        crate::state::AUTO_POPUP_DELAY_MS,
                    ))
                    .await;

                    // Check if this generation is still current
                    if manager.current_debounce_generation() != generation {
                        return;
                    }
                }
                // When active: trigger immediately (no delay) for live filtering

                // Send CommandEvent to trigger completion with proper buffer context
                manager.send_command_event(CommandEvent {
                    command: CommandRef::Registered(command_id::COMPLETION_TRIGGER),
                    context: CommandContext {
                        buffer_id,
                        window_id: 0,
                        count: None,
                    },
                });
            });

            EventResult::NotHandled
        });

        let _ = state; // Suppress unused warning
    }

    fn boot(
        &self,
        _bus: &EventBus,
        state: Arc<PluginStateRegistry>,
        event_tx: Option<tokio::sync::mpsc::Sender<reovim_core::event::RuntimeEvent>>,
    ) {
        // Get event_tx from parameter or fall back to state registry
        let Some(event_tx) = event_tx.or_else(|| state.inner_event_tx()) else {
            tracing::warn!("Completion plugin boot: event_tx not available");
            return;
        };

        // Store event_tx for auto-popup debounce (to send CommandEvent)
        self.manager.set_inner_event_tx(event_tx.clone());

        // Spawn the completion saturator
        let sources = self.manager.sources();
        let cache = Arc::clone(&self.manager.cache);

        let handle = spawn_completion_saturator(sources, cache, event_tx, 50);

        self.manager.set_saturator(handle);

        tracing::info!("Completion plugin booted with saturator");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_plugin_new() {
        let plugin = CompletionPlugin::new();
        assert_eq!(plugin.id().as_str(), "reovim:completion");
        assert_eq!(plugin.name(), "Completion");
    }

    #[test]
    fn test_completion_plugin_default() {
        let plugin = CompletionPlugin::default();
        assert_eq!(plugin.name(), "Completion");
    }

    #[test]
    fn test_completion_plugin_dependencies() {
        let plugin = CompletionPlugin::new();
        let deps = plugin.dependencies();
        assert!(deps.is_empty());
    }
}
