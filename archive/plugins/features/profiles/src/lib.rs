//! Profile management plugin for reovim
//!
//! This plugin registers the `:profile` ex-command with subcommands:
//! - `:profile list` - Open profile picker
//! - `:profile load <name>` - Load a profile by name
//! - `:profile save <name>` - Save current settings as a profile

use {
    reovim_core::{
        command_line::ExCommandHandler,
        event_bus::{
            DynEvent, EventBus, EventResult,
            core_events::{
                ProfileListEvent, ProfileLoadEvent, ProfileSaveEvent, RegisterExCommand,
            },
        },
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    std::{collections::HashMap, sync::Arc},
};

/// Profile management plugin
pub struct ProfilesPlugin;

impl Plugin for ProfilesPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("core:profiles")
    }

    fn build(&self, _ctx: &mut PluginContext) {
        // No build-time setup needed
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register :profile subcommands
        let mut subcommands = HashMap::new();

        // :profile list - Opens profile picker
        subcommands.insert(
            "list".to_string(),
            Box::new(ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(ProfileListEvent),
                description: "List all available profiles",
            }),
        );

        // :profile load <name> - Load a profile
        subcommands.insert(
            "load".to_string(),
            Box::new(ExCommandHandler::SingleArg {
                event_constructor: |name| DynEvent::new(ProfileLoadEvent { name }),
                description: "Load a profile by name",
            }),
        );

        // :profile save <name> - Save current settings
        subcommands.insert(
            "save".to_string(),
            Box::new(ExCommandHandler::SingleArg {
                event_constructor: |name| DynEvent::new(ProfileSaveEvent { name }),
                description: "Save current settings as a profile",
            }),
        );

        // Register the :profile command with all subcommands
        bus.emit(RegisterExCommand::new(
            "profile",
            ExCommandHandler::Subcommand {
                subcommands,
                description: "Manage configuration profiles",
            },
        ));

        tracing::debug!("ProfilesPlugin: Registered :profile command with subcommands");

        // Subscribe to ProfileListEvent to open the profile picker
        // TODO: Once profile picker UI is implemented, open it here
        bus.subscribe::<ProfileListEvent, _>(100, move |_event, ctx| {
            tracing::info!("ProfileListEvent received - profile picker UI not yet implemented");
            // Future: ctx.emit(MicroscopeOpen { picker_type: ProfilePicker });
            ctx.request_render();
            EventResult::Handled
        });

        tracing::debug!("ProfilesPlugin: Subscribed to ProfileListEvent");
    }
}
