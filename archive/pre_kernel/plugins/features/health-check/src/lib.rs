pub mod builtin;
pub mod check;
pub mod command;
pub mod manager;
pub mod window;

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    bind::{CommandRef, EditModeKind, KeymapScope},
    command::id::CommandId,
    command_line::ExCommandHandler,
    display::DisplayInfo,
    event_bus::{
        DynEvent, EventBus, EventResult,
        core_events::{RegisterExCommand, RequestFocusChange},
    },
    keys,
    modd::ComponentId,
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

// Re-export public API
pub use {
    check::{
        CheckStatus, HealthCheck, HealthCheckResult, HealthCheckResultWithMeta, RegisterHealthCheck,
    },
    command::{
        HealthCheckClose, HealthCheckOpen, HealthCheckRerun, HealthCheckSelectNext,
        HealthCheckSelectPrev, HealthCheckToggle,
    },
    manager::HealthCheckManager,
};

use {
    builtin::{
        EventBusCheck, FrameRendererCheck, KeybindingCheck, PluginSystemCheck, RuntimeCheck,
    },
    window::HealthCheckPluginWindow,
};

/// Plugin-local command IDs
pub mod command_id {
    use super::CommandId;

    pub const HEALTH_CHECK_OPEN: CommandId = CommandId::new("health_check_open");
    pub const HEALTH_CHECK_CLOSE: CommandId = CommandId::new("health_check_close");
    pub const HEALTH_CHECK_RERUN: CommandId = CommandId::new("health_check_rerun");
    pub const HEALTH_CHECK_SELECT_NEXT: CommandId = CommandId::new("health_check_select_next");
    pub const HEALTH_CHECK_SELECT_PREV: CommandId = CommandId::new("health_check_select_prev");
    pub const HEALTH_CHECK_TOGGLE: CommandId = CommandId::new("health_check_toggle");
}

/// Component ID for health check modal
pub const COMPONENT_ID: ComponentId = ComponentId("health_check");

/// Health check plugin
pub struct HealthCheckPlugin;

impl Default for HealthCheckPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckPlugin {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Plugin for HealthCheckPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:health-check")
    }

    fn name(&self) -> &'static str {
        "Health Check"
    }

    fn description(&self) -> &'static str {
        "System and plugin health status display"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register display info for status line
        use reovim_core::highlight::{Color, Style};

        // Teal/cyan for health diagnostics (medical theme)
        let teal = Color::Rgb {
            r: 38,
            g: 198,
            b: 218,
        };
        let fg = Color::Rgb {
            r: 33,
            g: 37,
            b: 43,
        };
        let style = Style::new().fg(fg).bg(teal).bold();

        ctx.register_display(COMPONENT_ID, DisplayInfo::new(" HEALTH ", " ", style));

        // Register commands
        let _ = ctx.register_command(HealthCheckOpen);
        let _ = ctx.register_command(HealthCheckClose);
        let _ = ctx.register_command(HealthCheckRerun);
        let _ = ctx.register_command(HealthCheckSelectNext);
        let _ = ctx.register_command(HealthCheckSelectPrev);
        let _ = ctx.register_command(HealthCheckToggle);

        // Register component-scoped keybindings
        let health_normal = KeymapScope::Component {
            id: COMPONENT_ID,
            mode: EditModeKind::Normal,
        };

        // Navigation
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys!['j'],
            CommandRef::Registered(command_id::HEALTH_CHECK_SELECT_NEXT),
        );
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys!['k'],
            CommandRef::Registered(command_id::HEALTH_CHECK_SELECT_PREV),
        );
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys![Down],
            CommandRef::Registered(command_id::HEALTH_CHECK_SELECT_NEXT),
        );
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys![Up],
            CommandRef::Registered(command_id::HEALTH_CHECK_SELECT_PREV),
        );

        // Re-run checks
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys!['r'],
            CommandRef::Registered(command_id::HEALTH_CHECK_RERUN),
        );

        // Toggle expansion
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys![Space],
            CommandRef::Registered(command_id::HEALTH_CHECK_TOGGLE),
        );
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys![Enter],
            CommandRef::Registered(command_id::HEALTH_CHECK_TOGGLE),
        );

        // Close modal
        ctx.bind_key_scoped(
            health_normal.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::HEALTH_CHECK_CLOSE),
        );
        ctx.bind_key_scoped(
            health_normal,
            keys!['q'],
            CommandRef::Registered(command_id::HEALTH_CHECK_CLOSE),
        );

        tracing::debug!("HealthCheckPlugin: registered commands and keybindings");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the health check manager
        registry.register(HealthCheckManager::new());

        // Register the plugin window
        registry.register_plugin_window(Arc::new(HealthCheckPluginWindow));

        tracing::info!("HealthCheckPlugin: registered plugin window and manager state");
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Register :health and :checkhealth ex-commands
        bus.emit(RegisterExCommand::new(
            "health",
            ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(HealthCheckOpen),
                description: "Open health check modal",
            },
        ));

        bus.emit(RegisterExCommand::new(
            "checkhealth",
            ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(HealthCheckOpen),
                description: "Open health check modal (Neovim alias)",
            },
        ));

        // Subscribe to RegisterHealthCheck events from other plugins
        {
            let state = Arc::clone(&state);
            bus.subscribe::<RegisterHealthCheck, _>(50, move |event, _ctx| {
                state.with_mut::<HealthCheckManager, _, _>(|mgr| {
                    mgr.register_check(Arc::clone(&event.check));
                });
                EventResult::Handled
            });
        }

        // Subscribe to HealthCheckOpen - open the modal and run checks
        {
            let state = Arc::clone(&state);
            bus.subscribe::<HealthCheckOpen, _>(100, move |_event, ctx| {
                state.with_mut::<HealthCheckManager, _, _>(|mgr| {
                    mgr.set_visible(true);
                    mgr.run_all_checks();
                });

                // Request focus change to health check component
                ctx.emit(RequestFocusChange {
                    target: COMPONENT_ID,
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Subscribe to HealthCheckClose - close the modal
        {
            let state = Arc::clone(&state);
            bus.subscribe::<HealthCheckClose, _>(100, move |_event, ctx| {
                state.with_mut::<HealthCheckManager, _, _>(|mgr| {
                    mgr.set_visible(false);
                });

                // Return focus to editor
                ctx.emit(RequestFocusChange {
                    target: ComponentId::EDITOR,
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Subscribe to HealthCheckRerun - re-run all checks
        {
            let state = Arc::clone(&state);
            bus.subscribe::<HealthCheckRerun, _>(100, move |_event, ctx| {
                state.with_mut::<HealthCheckManager, _, _>(|mgr| {
                    mgr.run_all_checks();
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Subscribe to HealthCheckSelectNext - navigate down
        {
            let state = Arc::clone(&state);
            bus.subscribe::<HealthCheckSelectNext, _>(100, move |_event, ctx| {
                state.with_mut::<HealthCheckManager, _, _>(|mgr| {
                    mgr.select_next();
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Subscribe to HealthCheckSelectPrev - navigate up
        {
            let state = Arc::clone(&state);
            bus.subscribe::<HealthCheckSelectPrev, _>(100, move |_event, ctx| {
                state.with_mut::<HealthCheckManager, _, _>(|mgr| {
                    mgr.select_prev();
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Subscribe to HealthCheckToggle - toggle expanded details
        {
            let state = Arc::clone(&state);
            bus.subscribe::<HealthCheckToggle, _>(100, move |_event, ctx| {
                state.with_mut::<HealthCheckManager, _, _>(|mgr| {
                    mgr.toggle_current();
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Auto-register built-in core health checks
        Self::register_builtin_checks(bus, &state);

        tracing::debug!("HealthCheckPlugin: subscribed to events and registered ex-commands");
    }
}

impl HealthCheckPlugin {
    /// Register built-in core health checks
    fn register_builtin_checks(bus: &EventBus, state: &Arc<PluginStateRegistry>) {
        // Check 1: Runtime check
        bus.emit(RegisterHealthCheck::new(Arc::new(RuntimeCheck)));

        // Check 2: Plugin system check
        bus.emit(RegisterHealthCheck::new(Arc::new(PluginSystemCheck {
            state: Arc::clone(state),
        })));

        // Check 3: Event bus check
        bus.emit(RegisterHealthCheck::new(Arc::new(EventBusCheck)));

        // Check 4: Frame renderer / terminal check
        bus.emit(RegisterHealthCheck::new(Arc::new(FrameRendererCheck)));

        // Check 5: Keybinding check
        bus.emit(RegisterHealthCheck::new(Arc::new(KeybindingCheck)));

        tracing::debug!("HealthCheckPlugin: registered {} built-in checks", 5);
    }
}
