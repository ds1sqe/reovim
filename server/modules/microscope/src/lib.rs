#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Microscope fuzzy finder module - POLICY layer.
//!
//! Orchestrates the fuzzy finder UI: provides session state management,
//! command handlers, and bridges state to clients.
//!
//! # Architecture (#522)
//!
//! This module reads from driver-picker's `PickerRegistry` and owns the
//! per-client `MicroscopeState` stored in `ExtensionMap`. The bridge
//! serializes state to JSON consumed by both TUI and Web extensions.
//!
//! Picker data sources (files, buffers, grep, commands) live in separate
//! `reovim-picker-*` crates. Each picker implements `Picker::execute()`
//! for its own action dispatch.

pub mod bridge;
pub mod commands;
pub mod ids;
pub mod modes;
pub mod resolver;
pub mod state;

pub use {bridge::MicroscopeBridge, state::MicroscopeState};

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

/// Microscope fuzzy finder module.
///
/// Registers [`MicroscopeBridge`] and commands, modes, and keybindings
/// for the fuzzy finder. Picker data sources are registered by separate
/// `reovim-picker-*` modules.
pub struct MicroscopeModule;

impl MicroscopeModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for MicroscopeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MicroscopeModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Microscope"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register MicroscopeBridge via BridgeProvider.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(MicroscopeBridge);

        // Register modes.
        let mode_store = ctx.services.get_or_create::<ModeInfoStore>();
        for mode in modes::MicroscopeMode::ALL {
            mode_store.add(ModeInfo::from_mode(*mode));
        }

        // Register resolver for microscope picker mode.
        let resolver_registry = ctx.services.get_or_create::<ResolverRegistry>();
        resolver_registry.register(resolver::MicroscopeResolver::new());

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

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Microscope mode: navigation
            KeybindingRegistration::new("<C-n>", ids::NEXT_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Next item"),
            KeybindingRegistration::new("<Down>", ids::NEXT_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Next item"),
            KeybindingRegistration::new("<C-p>", ids::PREV_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Previous item"),
            KeybindingRegistration::new("<Up>", ids::PREV_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Previous item"),
            // Microscope mode: actions
            KeybindingRegistration::new("<CR>", ids::SELECT_ITEM)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Select item"),
            KeybindingRegistration::new("<Esc>", ids::CLOSE)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Close picker"),
            KeybindingRegistration::new("<BS>", ids::BACKSPACE)
                .with_modes(&["microscope:MICROSCOPE"])
                .with_description("Delete character"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(MicroscopeModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id() {
        let module = MicroscopeModule::new();
        assert_eq!(module.id().as_str(), "microscope");
    }

    #[test]
    fn module_name() {
        let module = MicroscopeModule::new();
        assert_eq!(module.name(), "Microscope");
    }

    #[test]
    fn module_version() {
        let module = MicroscopeModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = MicroscopeModule::default();
        assert_eq!(module.id().as_str(), "microscope");
    }

    #[test]
    fn module_exit() {
        let mut module = MicroscopeModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn module_init_registers_bridge_and_services() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = MicroscopeModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        // Verify bridge was registered.
        let provider = services.get::<BridgeProvider>().unwrap();
        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].kind(), "microscope");

        // Verify modes were registered.
        let mode_store = services.get::<ModeInfoStore>();
        assert!(mode_store.is_some());
        let modes = mode_store.unwrap().take_modes();
        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].display_name, "MICROSCOPE");

        // Verify commands were registered.
        let command_store = services.get::<CommandHandlerStore>();
        assert!(command_store.is_some());

        // Verify keybindings were registered.
        let keybinding_store = services.get::<KeybindingStore>();
        assert!(keybinding_store.is_some());

        // Verify resolver was registered.
        let resolver_registry = services.get::<ResolverRegistry>();
        assert!(resolver_registry.is_some());
        let reg = resolver_registry.unwrap();
        assert!(reg.get(&modes::MicroscopeMode::PICKER_ID).is_some());
    }

    #[test]
    fn keybindings_not_empty() {
        let module = MicroscopeModule::new();
        let bindings = module.keybindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn keybindings_have_picker_mode_actions() {
        let module = MicroscopeModule::new();
        let bindings = module.keybindings();
        // C-n, Down, C-p, Up, CR, Esc, BS = 7
        let count = bindings
            .iter()
            .filter(|b| b.modes.contains(&"microscope:MICROSCOPE"))
            .count();
        assert_eq!(count, 7);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_context(
        services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> ModuleContext {
        ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services,
            std::path::PathBuf::from("/tmp"),
            std::path::PathBuf::from("/tmp"),
        )
    }
}
