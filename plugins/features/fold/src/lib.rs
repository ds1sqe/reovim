//! Code folding plugin for reovim
//!
//! This plugin provides code folding functionality:
//! - Toggle, open, close folds at cursor
//! - Open/close all folds in buffer
//! - Fold ranges from treesitter queries
//!
//! # State Management
//!
//! The plugin stores `FoldManager` in `PluginStateRegistry`, accessible by any component.
//!
//! # Event Bus Integration
//!
//! Commands emit fold events, and the plugin subscribes to handle them.

pub mod commands;
pub mod events;
pub mod stage;
pub mod state;

#[cfg(test)]
mod tests;

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    event_bus::{BufferClosed, EventBus, EventResult, FoldRangesComputed},
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

use state::SharedFoldManager;
use stage::FoldRenderStage;

use commands::{
    FoldCloseAllCommand, FoldCloseCommand, FoldOpenAllCommand, FoldOpenCommand, FoldToggleCommand,
};

/// Code folding plugin
///
/// Provides code folding with treesitter integration:
/// - `za` toggle fold
/// - `zo` open fold
/// - `zc` close fold
/// - `zR` open all folds
/// - `zM` close all folds
pub struct FoldPlugin {
    fold_manager: Arc<SharedFoldManager>,
}

impl Default for FoldPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl FoldPlugin {
    /// Create a new fold plugin
    pub fn new() -> Self {
        Self {
            fold_manager: Arc::new(SharedFoldManager::new()),
        }
    }
}

impl Plugin for FoldPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:fold")
    }

    fn name(&self) -> &'static str {
        "Fold"
    }

    fn description(&self) -> &'static str {
        "Code folding with treesitter integration"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        // CorePlugin dependency
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register commands
        let _ = ctx.register_command(FoldToggleCommand);
        let _ = ctx.register_command(FoldOpenCommand);
        let _ = ctx.register_command(FoldCloseCommand);
        let _ = ctx.register_command(FoldOpenAllCommand);
        let _ = ctx.register_command(FoldCloseAllCommand);

        // Register render stage for fold visibility transformations
        let stage = Arc::new(FoldRenderStage::new(Arc::clone(&self.fold_manager)));
        ctx.register_render_stage(stage);

        tracing::debug!("FoldPlugin: registered commands and render stage");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register shared fold manager as both type state and visibility source
        registry.register(Arc::clone(&self.fold_manager));
        registry.set_visibility_source(
            Arc::clone(&self.fold_manager) as Arc<dyn reovim_core::visibility::BufferVisibilitySource>
        );

        tracing::debug!("FoldPlugin: initialized SharedFoldManager");
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Handle fold toggle
        let state_clone = Arc::clone(&state);
        bus.subscribe::<FoldToggleEvent, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.toggle(event.buffer_id, event.line));
            });
            tracing::trace!(
                buffer_id = event.buffer_id,
                line = event.line,
                "FoldPlugin: toggled fold"
            );
            EventResult::Handled
        });

        // Handle fold open
        let state_clone = Arc::clone(&state);
        bus.subscribe::<FoldOpenEvent, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.open(event.buffer_id, event.line));
            });
            tracing::trace!(
                buffer_id = event.buffer_id,
                line = event.line,
                "FoldPlugin: opened fold"
            );
            EventResult::Handled
        });

        // Handle fold close
        let state_clone = Arc::clone(&state);
        bus.subscribe::<FoldCloseEvent, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.close(event.buffer_id, event.line));
            });
            tracing::trace!(
                buffer_id = event.buffer_id,
                line = event.line,
                "FoldPlugin: closed fold"
            );
            EventResult::Handled
        });

        // Handle open all folds
        let state_clone = Arc::clone(&state);
        bus.subscribe::<FoldOpenAllEvent, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.open_all(event.buffer_id));
            });
            tracing::trace!(buffer_id = event.buffer_id, "FoldPlugin: opened all folds");
            EventResult::Handled
        });

        // Handle close all folds
        let state_clone = Arc::clone(&state);
        bus.subscribe::<FoldCloseAllEvent, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.close_all(event.buffer_id));
            });
            tracing::trace!(buffer_id = event.buffer_id, "FoldPlugin: closed all folds");
            EventResult::Handled
        });

        // Handle fold ranges update (from plugin events)
        let state_clone = Arc::clone(&state);
        bus.subscribe::<FoldRangesUpdatedEvent, _>(50, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.set_ranges(event.buffer_id, event.ranges.clone()));
            });
            tracing::trace!(
                buffer_id = event.buffer_id,
                count = event.ranges.len(),
                "FoldPlugin: updated fold ranges (plugin event)"
            );
            EventResult::Handled
        });

        // Handle fold ranges computed by treesitter (core event)
        let state_clone = Arc::clone(&state);
        bus.subscribe::<FoldRangesComputed, _>(50, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.set_ranges(event.buffer_id, event.ranges.clone()));
            });
            tracing::trace!(
                buffer_id = event.buffer_id,
                count = event.ranges.len(),
                "FoldPlugin: updated fold ranges (treesitter)"
            );
            EventResult::Handled
        });

        // Handle buffer close - clean up fold state
        let state_clone = Arc::clone(&state);
        bus.subscribe::<BufferClosed, _>(100, move |event, _ctx| {
            state_clone.with::<Arc<SharedFoldManager>, _, _>(|fm| {
                fm.with_mut(|m| m.remove_buffer(event.buffer_id));
            });
            tracing::trace!(
                buffer_id = event.buffer_id,
                "FoldPlugin: cleaned up fold state for closed buffer"
            );
            EventResult::Handled
        });

        tracing::debug!("FoldPlugin: subscribed to fold events");
    }
}

// Re-export key types for external use
pub use events::{
    FoldCloseAllEvent, FoldCloseEvent, FoldOpenAllEvent, FoldOpenEvent, FoldRangesUpdatedEvent,
    FoldToggleEvent,
};
// Re-export fold types
// Data types (FoldKind, FoldRange) come from core (used by treesitter)
// State types (FoldState, FoldManager, SharedFoldManager) are plugin-owned
pub use {
    reovim_core::folding::{FoldKind, FoldRange},
    state::{FoldManager, FoldState},
};
// SharedFoldManager is already imported at the top for internal use
