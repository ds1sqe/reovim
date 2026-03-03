#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Range-finder module - POLICY layer.
//!
//! This module provides jump navigation and code folding:
//! - **Jump navigation**: `s{char}{char}` two-char search with label overlay
//! - **Code folding**: `za`/`zo`/`zc`/`zR`/`zM` fold commands
//!
//! # Architecture (#524)
//!
//! Jump state (`JumpSessionState`) and fold state (`FoldSessionState`) are
//! independent `SessionExtension` types. Jump labels are rendered by client
//! extensions via `ExtensionStateBridge`.
//!
//! The module registers its own mode (`range-finder:jump-input`) for label
//! selection, with `vim:normal` as parent for keybinding inheritance.

pub mod fold;
pub mod jump;

use reovim_kernel::api::v1::{
    KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};

const MODULE_ID: ModuleId = ModuleId::new("range-finder");

/// Range-finder module providing jump navigation and code folding.
pub struct RangeFinderModule;

impl RangeFinderModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RangeFinderModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for RangeFinderModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "range-finder"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register bridges (#524)
        let provider = ctx
            .services
            .get_or_create::<reovim_driver_session::bridges::BridgeProvider>();
        provider.register(jump::bridge::JumpBridge);
        provider.register(fold::bridge::FoldBridge);

        // Register jump commands (#524)
        let command_store = ctx
            .services
            .get_or_create::<reovim_driver_command::CommandHandlerStore>();
        for handler in jump::command::all_commands() {
            command_store.add(handler);
        }

        // Register fold commands (#524)
        for handler in fold::command::all_commands() {
            command_store.add(handler);
        }

        ProbeResult::Success
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // NOTE: Jump search keybinding ("s") is deferred until the command
            // execute() body is implemented. Registering a no-op stub for "s"
            // would swallow the key in vim:normal mode, breaking user input.
            //
            // Fold operations
            KeybindingRegistration::new("za", fold::ids::FOLD_TOGGLE)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Toggle fold at cursor"),
            KeybindingRegistration::new("zo", fold::ids::FOLD_OPEN)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Open fold at cursor"),
            KeybindingRegistration::new("zc", fold::ids::FOLD_CLOSE)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Close fold at cursor"),
            KeybindingRegistration::new("zR", fold::ids::FOLD_OPEN_ALL)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Open all folds"),
            KeybindingRegistration::new("zM", fold::ids::FOLD_CLOSE_ALL)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Close all folds"),
        ]
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(RangeFinderModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = RangeFinderModule::new();
        assert_eq!(module.id().as_str(), "range-finder");
    }

    #[test]
    fn test_module_name() {
        let module = RangeFinderModule::new();
        assert_eq!(module.name(), "range-finder");
    }

    #[test]
    fn test_module_version() {
        let module = RangeFinderModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_module_default() {
        let module = RangeFinderModule::default();
        assert_eq!(module.id().as_str(), "range-finder");
    }

    #[test]
    fn test_module_exit() {
        let mut module = RangeFinderModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_module_init() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = RangeFinderModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        // Verify bridges were registered (jump + fold = 2)
        let provider = services
            .get::<reovim_driver_session::bridges::BridgeProvider>()
            .unwrap();
        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 2);
        assert_eq!(bridges[0].kind(), "range-finder-jump");
        assert_eq!(bridges[1].kind(), "range-finder-fold");

        // Verify commands were registered (2 jump + 5 fold = 7)
        let command_store = services
            .get::<reovim_driver_command::CommandHandlerStore>()
            .unwrap();
        let handlers = command_store.take_handlers();
        assert_eq!(handlers.len(), 7);
    }

    #[test]
    fn test_keybindings() {
        let module = RangeFinderModule::new();
        let bindings = module.keybindings();
        assert_eq!(bindings.len(), 5); // za/zo/zc/zR/zM (s deferred)

        // Fold
        assert_eq!(bindings[0].keys, "za");
        assert_eq!(bindings[0].command_id, crate::fold::ids::FOLD_TOGGLE);
        assert_eq!(bindings[0].category, Some("folding"));

        assert_eq!(bindings[1].keys, "zo");
        assert_eq!(bindings[1].command_id, crate::fold::ids::FOLD_OPEN);

        assert_eq!(bindings[2].keys, "zc");
        assert_eq!(bindings[2].command_id, crate::fold::ids::FOLD_CLOSE);

        assert_eq!(bindings[3].keys, "zR");
        assert_eq!(bindings[3].command_id, crate::fold::ids::FOLD_OPEN_ALL);

        assert_eq!(bindings[4].keys, "zM");
        assert_eq!(bindings[4].command_id, crate::fold::ids::FOLD_CLOSE_ALL);
    }

    /// Create a minimal `ModuleContext` for testing.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_context(
        services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> ModuleContext {
        use {
            parking_lot::RwLock,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(reovim_driver_buffer::TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            services.clone(),
        );
        ModuleContext::new(
            kernel,
            services,
            std::path::PathBuf::from("/tmp"),
            std::path::PathBuf::from("/tmp"),
        )
    }
}
