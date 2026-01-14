//! Sticky context headers plugin
//!
//! Displays 1-3 enclosing scope headers pinned at the top of the viewport
//! as the user scrolls through large files.
//!
//! This plugin subscribes to `ViewportContextUpdated` events from the context
//! plugin and caches the context for rendering, avoiding redundant computation.

use std::sync::Arc;

use {
    reovim_core::{
        event_bus::{EventBus, EventResult},
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_context::ViewportContextUpdated,
};

mod state;
mod window;

use {
    state::{CachedViewportContext, StickyContextState},
    window::StickyContextWindow,
};

/// Sticky context headers plugin
pub struct StickyContextPlugin {
    state: Arc<StickyContextState>,
}

impl StickyContextPlugin {
    /// Create a new sticky context plugin
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(StickyContextState::new()),
        }
    }
}

impl Default for StickyContextPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for StickyContextPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:sticky-context")
    }

    fn name(&self) -> &'static str {
        "sticky-context"
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register plugin state
        registry.register(self.state.clone());

        // Register plugin window for rendering
        registry.register_plugin_window(Arc::new(StickyContextWindow::new(self.state.clone())));

        tracing::debug!("StickyContextPlugin: initialized");
    }

    fn build(&self, _ctx: &mut PluginContext) {
        tracing::debug!("StickyContextPlugin::build");
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register settings
        Self::register_settings(bus);

        // Subscribe to viewport context updates from context plugin
        let plugin_state = Arc::clone(&self.state);
        bus.subscribe::<ViewportContextUpdated, _>(100, move |event, _ctx| {
            // Cache the context for rendering
            plugin_state.set_cached_context(CachedViewportContext {
                buffer_id: event.buffer_id,
                context: event.context.clone(),
            });

            tracing::trace!(
                window_id = event.window_id,
                buffer_id = event.buffer_id,
                top_line = event.top_line,
                has_context = event.context.is_some(),
                "StickyContextPlugin: received ViewportContextUpdated"
            );

            EventResult::Handled
        });

        tracing::debug!("StickyContextPlugin::subscribe - subscribed to ViewportContextUpdated");
    }
}

impl StickyContextPlugin {
    /// Register plugin settings
    fn register_settings(bus: &EventBus) {
        use reovim_core::option::{OptionCategory, OptionSpec, OptionValue, RegisterOption};

        // Enable/disable sticky headers
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "sticky_headers_enabled",
                "Enable sticky context headers",
                OptionValue::Bool(true),
            )
            .with_category(OptionCategory::Display)
            .with_section("Editor")
            .with_display_order(55),
        ));

        // Max number of headers to show
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "sticky_headers_max_count",
                "Maximum sticky headers to display",
                OptionValue::Integer(3),
            )
            .with_category(OptionCategory::Display)
            .with_section("Editor")
            .with_display_order(56),
        ));

        // Show separator line
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "sticky_headers_show_separator",
                "Show separator line below sticky headers",
                OptionValue::Bool(true),
            )
            .with_category(OptionCategory::Display)
            .with_section("Editor")
            .with_display_order(57),
        ));
    }
}
