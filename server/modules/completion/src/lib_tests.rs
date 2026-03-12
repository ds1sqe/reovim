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
