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
