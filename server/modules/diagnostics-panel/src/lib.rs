#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Diagnostics panel module for reovim.
//!
//! Provides a navigable diagnostics list (trouble-like) with filtering,
//! sorting, and jump-to-location. Follows the same extension-state-bridge
//! pattern as `microscope`.

pub mod bridge;
pub mod commands;
pub mod ids;
pub mod items;
mod keybinding;
pub mod modes;
pub mod resolver;
pub mod state;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

const KIND: &str = "diagnostics-panel";

/// Mode string for keybinding registration.
const PANEL_MODE_STR: &str = "diagnostics-panel:DIAGNOSTICS";

/// Diagnostics panel module.
///
/// Registers commands, modes, keybindings, and the diagnostics bridge
/// for the navigable diagnostics panel.
pub struct DiagnosticsPanelModule;

impl DiagnosticsPanelModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for DiagnosticsPanelModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for DiagnosticsPanelModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Diagnostics Panel"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register bridge.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(bridge::DiagnosticsPanelBridge);

        // Register modes.
        let mode_store = ctx.services.get_or_create::<ModeInfoStore>();
        for mode in modes::DiagnosticsPanelMode::ALL {
            mode_store.add(ModeInfo::from_mode(*mode));
        }

        // Register resolver.
        let resolver_registry = ctx.services.get_or_create::<ResolverRegistry>();
        resolver_registry.register(resolver::DiagnosticsPanelResolver::new());

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register keybindings.
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        // Normal-mode bindings from personality adapter (#700)
        let mut bindings = keybinding::all();
        // Panel-mode bindings (module-owned, already qualified)
        bindings.extend([
            // Navigation
            KeybindingRegistration::new("j", ids::TROUBLE_NEXT)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Next item"),
            KeybindingRegistration::new("<Down>", ids::TROUBLE_NEXT)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Next item"),
            KeybindingRegistration::new("k", ids::TROUBLE_PREV)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Previous item"),
            KeybindingRegistration::new("<Up>", ids::TROUBLE_PREV)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Previous item"),
            // Actions
            KeybindingRegistration::new("<CR>", ids::TROUBLE_SELECT)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Jump to diagnostic"),
            KeybindingRegistration::new("q", ids::TROUBLE_CLOSE)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Close panel"),
            KeybindingRegistration::new("<Esc>", ids::TROUBLE_CLOSE)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Close panel"),
            // Filtering
            KeybindingRegistration::new("e", ids::TROUBLE_FILTER_ERROR)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Show only errors"),
            KeybindingRegistration::new("w", ids::TROUBLE_FILTER_WARNING)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Show only warnings"),
            KeybindingRegistration::new("a", ids::TROUBLE_FILTER_ALL)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Show all severities"),
            // Sort and refresh
            KeybindingRegistration::new("s", ids::TROUBLE_SORT)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Cycle sort order"),
            KeybindingRegistration::new("r", ids::TROUBLE_REFRESH)
                .with_modes(&[PANEL_MODE_STR])
                .with_description("Refresh diagnostics"),
        ]);
        bindings
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(DiagnosticsPanelModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
