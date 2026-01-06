//! Command-line auto-completion plugin for reovim
//!
//! Provides TAB completion for command mode (`:` commands):
//! - Command name completion with descriptions
//! - File path completion for :e, :w commands
//!
//! # Architecture
//!
//! - Synchronous completion (fast enough without background processing)
//! - Lock-free cache for responsive rendering (ArcSwap pattern)
//! - Wildmenu-style popup with icons and descriptions

mod cache;
mod command;
mod source;
mod window;

use std::{any::TypeId, sync::Arc};

pub use {
    cache::{
        CmdlineCompletionCache, CmdlineCompletionItem, CmdlineCompletionKind,
        CmdlineCompletionSnapshot,
    },
    command::{
        CmdlineComplete, CmdlineCompleteConfirm, CmdlineCompleteDismiss, CmdlineCompleteNext,
        CmdlineCompletePrev,
    },
    source::{complete_commands, complete_paths},
    window::CmdlineCompletionWindow,
};

use reovim_core::{
    command_line::CmdlineCompletionContext,
    event_bus::{EventBus, EventResult, core_events},
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

/// Shared state for command-line completion
pub struct CmdlineCompletionState {
    /// Lock-free cache for completion items
    pub cache: Arc<CmdlineCompletionCache>,
}

impl CmdlineCompletionState {
    /// Create a new state
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(CmdlineCompletionCache::new()),
        }
    }
}

impl Default for CmdlineCompletionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Command-line completion plugin
pub struct CmdlineCompletionPlugin {
    state: Arc<CmdlineCompletionState>,
}

impl CmdlineCompletionPlugin {
    /// Create a new plugin instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(CmdlineCompletionState::new()),
        }
    }
}

impl Default for CmdlineCompletionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CmdlineCompletionPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:cmdline-completion")
    }

    fn name(&self) -> &'static str {
        "Command-line Completion"
    }

    fn description(&self) -> &'static str {
        "TAB completion for command-line mode"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register commands
        let _ = ctx.register_command(CmdlineComplete);
        let _ = ctx.register_command(CmdlineCompleteNext);
        let _ = ctx.register_command(CmdlineCompletePrev);
        let _ = ctx.register_command(CmdlineCompleteConfirm);
        let _ = ctx.register_command(CmdlineCompleteDismiss);

        // Key bindings are registered in core bind/mod.rs for Command mode
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the shared state for cross-plugin access
        registry.register(Arc::clone(&self.state));

        // Register the plugin window
        registry.register_plugin_window(Arc::new(CmdlineCompletionWindow::new(Arc::clone(
            &self.state.cache,
        ))));
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Subscribe to completion trigger event (Tab key in command mode)
        let cache = Arc::clone(&self.state.cache);
        let bus_sender = bus.sender();
        bus.subscribe::<core_events::CmdlineCompletionTriggered, _>(100, move |event, ctx| {
            // Check if completion is already active - if so, go to next item
            if cache.is_active() {
                cache.select_next();
                ctx.request_render();
                return EventResult::Handled;
            }

            // Generate completions based on context
            let items = match &event.context {
                CmdlineCompletionContext::Command { prefix } => {
                    complete_commands(prefix, &event.registry_commands)
                }
                CmdlineCompletionContext::Argument {
                    command,
                    prefix,
                    arg_start,
                } => {
                    // Check if the command accepts file paths
                    let path_commands = [
                        "e", "edit", "w", "write", "sp", "split", "vs", "vsplit", "tabnew",
                    ];
                    if path_commands.contains(&command.as_str()) {
                        complete_paths(prefix, *arg_start)
                    } else {
                        Vec::new()
                    }
                }
            };

            if items.is_empty() {
                return EventResult::Handled; // Nothing to complete
            }

            // Get the replace_start position
            let replace_start = match &event.context {
                CmdlineCompletionContext::Command { .. } => 0,
                CmdlineCompletionContext::Argument { arg_start, .. } => *arg_start,
            };

            // Get the prefix for display
            let prefix = match &event.context {
                CmdlineCompletionContext::Command { prefix } => prefix.clone(),
                CmdlineCompletionContext::Argument { prefix, .. } => prefix.clone(),
            };

            // Store in cache
            cache.store(CmdlineCompletionSnapshot {
                items,
                selected_index: 0,
                active: true,
                prefix,
                replace_start,
            });

            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to CmdlineCompleteNext command
        let cache = Arc::clone(&self.state.cache);
        bus.subscribe::<CmdlineCompleteNext, _>(100, move |_event, ctx| {
            if cache.is_active() {
                cache.select_next();
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Subscribe to completion prev event (Shift-Tab)
        let cache = Arc::clone(&self.state.cache);
        bus.subscribe::<core_events::CmdlineCompletionPrevRequested, _>(100, move |_event, ctx| {
            if cache.is_active() {
                cache.select_prev();
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Subscribe to CmdlineCompletePrev command (alternative route)
        let cache = Arc::clone(&self.state.cache);
        bus.subscribe::<CmdlineCompletePrev, _>(100, move |_event, ctx| {
            if cache.is_active() {
                cache.select_prev();
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Subscribe to CmdlineCompleteConfirm command
        let cache = Arc::clone(&self.state.cache);
        let bus_sender_confirm = bus.sender();
        bus.subscribe::<CmdlineCompleteConfirm, _>(100, move |_event, ctx| {
            if !cache.is_active() {
                return EventResult::NotHandled;
            }

            let snapshot = cache.load();
            let Some(item) = snapshot.items.get(snapshot.selected_index) else {
                cache.dismiss();
                return EventResult::NotHandled;
            };

            // Emit event to apply the completion
            bus_sender_confirm.try_send(core_events::RequestApplyCmdlineCompletion {
                text: item.insert_text.clone(),
                replace_start: snapshot.replace_start,
            });

            cache.dismiss();
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to CmdlineCompleteDismiss command
        let cache = Arc::clone(&self.state.cache);
        bus.subscribe::<CmdlineCompleteDismiss, _>(100, move |_event, ctx| {
            if cache.is_active() {
                cache.dismiss();
                ctx.request_render();
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        });

        // Subscribe to ModeChanged to dismiss completion when leaving command mode
        let cache = Arc::clone(&self.state.cache);
        bus.subscribe::<core_events::ModeChanged, _>(100, move |event, ctx| {
            // Dismiss if leaving command mode
            if !event.to.contains("Command") && cache.is_active() {
                cache.dismiss();
                ctx.request_render();
            }
            EventResult::NotHandled // Don't consume the event
        });

        // Note: Completion is dismissed via ModeChanged handler when leaving command mode
        // (which happens after execute/cancel)

        let _ = state; // Suppress unused warning
        let _ = bus_sender; // Suppress unused warning (used in closure above)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_new() {
        let plugin = CmdlineCompletionPlugin::new();
        assert_eq!(plugin.id().as_str(), "reovim:cmdline-completion");
        assert_eq!(plugin.name(), "Command-line Completion");
    }

    #[test]
    fn test_plugin_default() {
        let plugin = CmdlineCompletionPlugin::default();
        assert_eq!(plugin.name(), "Command-line Completion");
    }

    #[test]
    fn test_plugin_dependencies() {
        let plugin = CmdlineCompletionPlugin::new();
        assert!(plugin.dependencies().is_empty());
    }
}
