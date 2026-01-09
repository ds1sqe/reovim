//! Context plugin for reovim
//!
//! Provides event-driven context updates for UI components.
//! Instead of consumers polling for context on every frame, this plugin:
//!
//! 1. Subscribes to core events (`CursorMoved`, `ViewportScrolled`, `BufferModified`)
//! 2. Computes context using the registered `ContextProvider`
//! 3. Emits context update events (`CursorContextUpdated`, `ViewportContextUpdated`)
//! 4. Consumers subscribe to these events and cache the results
//!
//! This reduces redundant computation and improves performance.

pub mod events;
pub mod manager;

use std::sync::Arc;

pub use {
    events::{CursorContextUpdated, ViewportContextUpdated},
    manager::{CachedContext, ContextManager, SharedContextManager},
};

use reovim_core::{
    event_bus::{BufferModified, CursorMoved, EventBus, EventResult, ViewportScrolled},
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

/// Context plugin for event-driven context updates
pub struct ContextPlugin {
    manager: SharedContextManager,
}

impl Default for ContextPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextPlugin {
    /// Create a new context plugin
    #[must_use]
    pub fn new() -> Self {
        Self {
            manager: Arc::new(ContextManager::new()),
        }
    }
}

impl Plugin for ContextPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:context")
    }

    fn name(&self) -> &'static str {
        "Context"
    }

    fn description(&self) -> &'static str {
        "Event-driven context updates for UI components"
    }

    fn build(&self, _ctx: &mut PluginContext) {
        tracing::debug!("ContextPlugin: built");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register shared context manager for other plugins to access cached context
        registry.register(Arc::clone(&self.manager));
        tracing::debug!("ContextPlugin: initialized SharedContextManager");
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Handle cursor movement - compute and emit cursor context
        let manager = Arc::clone(&self.manager);
        let state_for_cursor = Arc::clone(&state);
        bus.subscribe::<CursorMoved, _>(100, move |event, ctx| {
            let buffer_id = event.buffer_id;
            #[allow(clippy::cast_possible_truncation)]
            let line = event.to.0 as u32;
            #[allow(clippy::cast_possible_truncation)]
            let col = event.to.1 as u32;

            // Check if cursor actually changed position
            if !manager.cursor_changed(buffer_id, line, col) {
                return EventResult::Handled;
            }
            manager.update_cursor(buffer_id, line, col);

            // Get context from the registry's context provider
            // Note: content is cached in treesitter manager, so we pass empty string
            // The provider will use its own cached source
            let context = state_for_cursor.get_context(buffer_id, line, col, "");

            // Cache the result
            manager.set_cursor_context(CachedContext {
                buffer_id,
                line,
                col,
                context: context.clone(),
            });

            // Emit context update event
            ctx.emit(CursorContextUpdated {
                buffer_id,
                line,
                col,
                context,
            });

            tracing::trace!(buffer_id, line, col, "ContextPlugin: emitted CursorContextUpdated");

            EventResult::Handled
        });

        // Handle viewport scroll - compute and emit viewport context
        let manager = Arc::clone(&self.manager);
        let state_for_viewport = Arc::clone(&state);
        bus.subscribe::<ViewportScrolled, _>(100, move |event, ctx| {
            let buffer_id = event.buffer_id;
            let top_line = event.top_line;

            tracing::debug!(
                buffer_id,
                top_line,
                window_id = event.window_id,
                "ContextPlugin: received ViewportScrolled"
            );

            // Check if viewport actually changed
            if !manager.viewport_changed(buffer_id, top_line) {
                tracing::debug!(buffer_id, top_line, "ContextPlugin: viewport unchanged, skipping");
                return EventResult::Handled;
            }
            manager.update_viewport(buffer_id, top_line);

            // Get context at the top line of the viewport
            // This gives us the enclosing scopes that should be shown as sticky headers
            let context = state_for_viewport.get_context(buffer_id, top_line, 0, "");
            let has_context = context.is_some();

            // Cache the result
            manager.set_viewport_context(CachedContext {
                buffer_id,
                line: top_line,
                col: 0,
                context: context.clone(),
            });

            // Emit context update event
            ctx.emit(ViewportContextUpdated {
                window_id: event.window_id,
                buffer_id,
                top_line,
                context,
            });

            tracing::debug!(
                buffer_id,
                top_line,
                window_id = event.window_id,
                has_context,
                "ContextPlugin: emitted ViewportContextUpdated"
            );

            EventResult::Handled
        });

        // Handle buffer modification - invalidate cached context
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<BufferModified, _>(100, move |event, _ctx| {
            manager.invalidate_buffer(event.buffer_id);
            tracing::trace!(
                buffer_id = event.buffer_id,
                "ContextPlugin: invalidated context cache"
            );
            EventResult::Handled
        });

        tracing::debug!("ContextPlugin: subscribed to cursor, viewport, and buffer events");
    }
}
