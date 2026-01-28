//! Statusline extension plugin for reovim
//!
//! This plugin provides a section-based API for other plugins to contribute
//! content to the status line.
//!
//! # Usage
//!
//! Other plugins can register sections by emitting `StatuslineSectionRegister`:
//!
//! ```ignore
//! use reovim_plugin_statusline::{
//!     StatuslineSectionRegister, StatuslineSection, SectionContent
//! };
//! use reovim_core::plugin::SectionAlignment;
//!
//! bus.emit(StatuslineSectionRegister {
//!     section: StatuslineSection::new(
//!         "my_plugin_status",
//!         100, // priority
//!         SectionAlignment::Right,
//!         |ctx| SectionContent::new(" LSP ")
//!     ),
//! });
//! ```

pub mod command;
pub mod context_section;
pub mod section;
pub mod state;

use std::{any::TypeId, sync::Arc};

use {
    reovim_core::{
        event_bus::{EventBus, EventResult},
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_context::CursorContextUpdated,
};

use {
    command::{StatuslineSectionRegister, StatuslineSectionUnregister},
    state::{CachedCursorContext, SharedStatuslineManager},
};

// Re-export for external use
pub use {
    command::StatuslineRefresh,
    section::{SectionContent, SectionRenderContext, StatuslineSection},
    state::StatuslineManagerHandle,
};

/// Statusline extension plugin
///
/// Provides a section-based API for plugins to contribute to the status line.
pub struct StatuslinePlugin {
    manager: Arc<SharedStatuslineManager>,
}

impl Default for StatuslinePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl StatuslinePlugin {
    /// Create a new statusline plugin
    #[must_use]
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SharedStatuslineManager::new()),
        }
    }
}

impl Plugin for StatuslinePlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:statusline")
    }

    fn name(&self) -> &'static str {
        "Statusline"
    }

    fn description(&self) -> &'static str {
        "Extensible statusline with section-based API"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register refresh command
        let _ = ctx.register_command(StatuslineRefresh);

        tracing::debug!("StatuslinePlugin: registered commands");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        use reovim_core::plugin::StatuslineSectionProvider;

        // Register the manager in plugin state for other plugins to access
        registry.register(Arc::clone(&self.manager));

        // Register as the statusline provider (cast to trait object)
        registry.set_statusline_provider(
            Arc::clone(&self.manager) as Arc<dyn StatuslineSectionProvider>
        );

        tracing::debug!("StatuslinePlugin: registered as statusline provider");
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        use reovim_core::option::{
            OptionCategory, OptionConstraint, OptionSpec, OptionValue, RegisterOption,
        };

        // Register settings
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "context_breadcrumb_enabled",
                "Show context breadcrumb in statusline",
                OptionValue::Bool(true),
            )
            .with_category(OptionCategory::Display)
            .with_section("Statusline")
            .with_display_order(50),
        ));

        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "context_separator",
                "Context breadcrumb separator",
                OptionValue::String(" > ".into()),
            )
            .with_category(OptionCategory::Display)
            .with_section("Statusline")
            .with_display_order(51),
        ));

        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "context_max_items",
                "Maximum breadcrumb items",
                OptionValue::Integer(4),
            )
            .with_category(OptionCategory::Display)
            .with_section("Statusline")
            .with_display_order(52)
            .with_constraint(OptionConstraint::range(1, 10)),
        ));

        // Auto-register context breadcrumb section
        bus.emit(StatuslineSectionRegister {
            section: context_section::create_context_section(),
        });

        // Subscribe to section registration events
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<StatuslineSectionRegister, _>(100, move |event, ctx| {
            manager.register_section(event.section.clone());
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to section unregistration events
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<StatuslineSectionUnregister, _>(100, move |event, ctx| {
            manager.unregister_section(event.id);
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to refresh command
        let _manager = Arc::clone(&self.manager);
        bus.subscribe::<StatuslineRefresh, _>(100, move |_event, ctx| {
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to cursor context updates from context plugin
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<CursorContextUpdated, _>(100, move |event, ctx| {
            // Cache the context for rendering
            manager.set_cached_cursor_context(CachedCursorContext {
                buffer_id: event.buffer_id,
                line: event.line,
                col: event.col,
                context: event.context.clone(),
            });

            // Request render to update statusline
            ctx.request_render();

            tracing::trace!(
                buffer_id = event.buffer_id,
                line = event.line,
                col = event.col,
                has_context = event.context.is_some(),
                "StatuslinePlugin: received CursorContextUpdated"
            );

            EventResult::Handled
        });

        tracing::debug!("StatuslinePlugin: subscribed to events");
    }
}
