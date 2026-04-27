use std::{path::PathBuf, sync::Arc};

use reovim_kernel::api::v1::{
    KernelContext, Module, ModuleContext, OptionSpec, OptionValue, ProbeResult, ServiceRegistry,
    Version,
};

use super::*;

#[test]
fn test_module_default() {
    let module = <FormatModule as Default>::default();
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_module_id() {
    let module = FormatModule::new();
    assert_eq!(module.id().as_str(), "format");
}

#[test]
fn test_module_name() {
    let module = FormatModule::new();
    assert_eq!(module.name(), "Format");
}

#[test]
fn test_module_version() {
    let module = FormatModule::new();
    assert_eq!(module.version(), Version::new(0, 1, 0));
}

#[test]
fn test_module_provides() {
    let module = FormatModule::new();
    let caps = module.provides();
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0], reovim_domain_text_capabilities::FORMATTER_PROVIDER);
}

#[test]
fn test_module_init() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = FormatModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify autoformat option was registered
    let value = ctx
        .kernel
        .options
        .get("autoformat", reovim_kernel::api::v1::OptionScopeId::Global);
    assert_eq!(value, Some(OptionValue::bool(true)));

    // Verify command handlers were registered
    let store = services
        .get::<reovim_driver_command::CommandHandlerStore>()
        .unwrap();
    let handlers = store.take_handlers();
    assert_eq!(handlers.len(), 2);

    // Verify subscription was created
    assert_eq!(module.subscriptions.len(), 1);
}

#[test]
fn test_module_init_duplicate_option() {
    let kernel = KernelContext::default();
    // Pre-register autoformat to trigger conflict
    kernel
        .options
        .register(OptionSpec::new("autoformat", "Already taken", OptionValue::bool(false)))
        .unwrap();

    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services,
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = FormatModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Failed(_)), "init should fail on duplicate option");
}

#[test]
fn test_module_exit() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services,
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = FormatModule::new();
    module.init(&ctx);
    assert!(!module.subscriptions.is_empty());

    let result = module.exit();
    assert!(result.is_ok());
    assert!(module.subscriptions.is_empty());
}
