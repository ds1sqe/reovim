#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! LSP navigation module for reovim.
//!
//! Provides `gd` (goto definition) and `gr` (find references) commands.
//! Single definition results jump directly; multiple results and all
//! references open in the microscope picker.

pub mod commands;
pub mod hover_bridge;
pub mod hover_state;
pub mod ids;
mod picker;
pub mod signature_help_bridge;
pub mod signature_help_state;

use std::sync::Arc;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_picker::PickerRegistry,
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

use picker::LspLocationPicker;

/// LSP navigation module.
///
/// Registers command handlers for `gd` and `gr`, and the
/// `lsp-locations` picker in `PickerRegistry`.
pub struct LspNavigationModule;

impl LspNavigationModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LspNavigationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LspNavigationModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "LSP Navigation"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register the lsp-locations picker.
        let picker_registry = ctx.services.get_or_create::<PickerRegistry>();
        picker_registry.register(Arc::new(LspLocationPicker::new()));

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register extension bridges for state serialization.
        let bridge_provider = ctx.services.get_or_create::<BridgeProvider>();
        bridge_provider.register(hover_bridge::HoverBridge);
        bridge_provider.register(signature_help_bridge::SignatureHelpBridge);

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(LspNavigationModule);

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn module_id() {
        let module = LspNavigationModule::new();
        assert_eq!(module.id().as_str(), "lsp-navigation");
    }

    #[test]
    fn module_name() {
        let module = LspNavigationModule::new();
        assert_eq!(module.name(), "LSP Navigation");
    }

    #[test]
    fn module_version() {
        let module = LspNavigationModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = LspNavigationModule::default();
        assert_eq!(module.id().as_str(), "lsp-navigation");
    }

    #[test]
    fn module_exit() {
        let mut module = LspNavigationModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn module_init_registers_picker_and_commands() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = LspNavigationModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        // Verify picker was registered.
        let picker_registry = services.get::<PickerRegistry>();
        assert!(picker_registry.is_some());
        let reg = picker_registry.unwrap();
        assert!(reg.get("lsp-locations").is_some());

        // Verify command handlers were registered.
        let command_store = services.get::<CommandHandlerStore>();
        assert!(command_store.is_some());

        // Verify extension bridges were registered.
        let bridge_provider = services.get::<BridgeProvider>();
        assert!(bridge_provider.is_some());
        let bridges = bridge_provider.unwrap().take_bridges();
        assert!(bridges.iter().any(|b| b.kind() == "hover"));
        assert!(bridges.iter().any(|b| b.kind() == "signature-help"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_context(
        services: Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> ModuleContext {
        ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services,
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
        )
    }
}
