#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Completion module - POLICY layer.
//!
//! Orchestrates the completion popup: registers completion sources, provides
//! session state management, and bridges state to clients.
//!
//! # Architecture (#521)
//!
//! This module owns the per-client `CompletionState` stored in `ExtensionMap`.
//! The bridge serializes state to JSON consumed by both TUI and Web extensions.
//! Completion sources are registered in the `CompletionSourceRegistry` service.

pub mod bridge;
pub mod buffer_words;
pub mod commands;
pub mod ids;
pub mod lsp_source;
pub mod notification_queue;
pub mod state;

pub use {bridge::CompletionBridge, state::CompletionState};

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_completion::CompletionSourceRegistry,
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    std::sync::Arc,
};

/// Completion module.
///
/// Registers [`CompletionBridge`], creates the [`CompletionSourceRegistry`]
/// service, and registers the [`BufferWordsSource`](buffer_words::BufferWordsSource)
/// during `init()`.
pub struct CompletionModule;

impl CompletionModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CompletionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CompletionModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Completion"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register CompletionBridge via BridgeProvider.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(CompletionBridge);

        // Register built-in completion sources.
        let registry = ctx.services.get_or_create::<CompletionSourceRegistry>();
        registry.register(Arc::new(buffer_words::BufferWordsSource));

        // Register LspCompletionSource in both registries:
        // - CompletionSourceRegistry: so the engine calls complete() on it
        // - ServiceRegistry: so fire_lsp_completion() can update_cache() on it
        let lsp_source = Arc::new(lsp_source::LspCompletionSource::new());
        registry.register(
            Arc::clone(&lsp_source) as Arc<dyn reovim_driver_completion::CompletionSource>
        );
        ctx.services.register(lsp_source);

        // Register pending notification queue for background thread → UI bridge.
        let _ = ctx
            .services
            .get_or_create::<notification_queue::PendingNotificationQueue>();

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CompletionModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id() {
        let module = CompletionModule::new();
        assert_eq!(module.id().as_str(), "completion");
    }

    #[test]
    fn module_name() {
        let module = CompletionModule::new();
        assert_eq!(module.name(), "Completion");
    }

    #[test]
    fn module_version() {
        let module = CompletionModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = CompletionModule::default();
        assert_eq!(module.id().as_str(), "completion");
    }

    #[test]
    fn module_exit() {
        let mut module = CompletionModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn module_init_registers_bridge_and_registry() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = CompletionModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        // Verify bridge was registered.
        let provider = services.get::<BridgeProvider>().unwrap();
        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].kind(), "completion");

        // Verify CompletionSourceRegistry was created with built-in sources.
        let registry = services.get::<CompletionSourceRegistry>();
        assert!(registry.is_some());
        let reg = registry.unwrap();
        assert_eq!(reg.len(), 2);
        assert!(reg.get("buffer").is_some());
        assert!(reg.get("lsp").is_some());

        // Verify LspCompletionSource is also in ServiceRegistry (for cache updates).
        let lsp_source = services.get::<lsp_source::LspCompletionSource>();
        assert!(lsp_source.is_some());

        // Verify PendingNotificationQueue was registered.
        let queue = services.get::<notification_queue::PendingNotificationQueue>();
        assert!(queue.is_some());

        // Verify commands were registered.
        let command_store = services.get::<CommandHandlerStore>();
        assert!(command_store.is_some());
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
