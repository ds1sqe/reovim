use super::*;

#[test]
fn test_module_id() {
    let module = ClipboardModule::new();
    assert_eq!(module.id().as_str(), "clipboard");
}

#[test]
fn test_module_name() {
    let module = ClipboardModule::new();
    assert_eq!(module.name(), "Clipboard");
}

#[test]
fn test_module_version() {
    let module = ClipboardModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_new = ClipboardModule::new();
    let from_default: ClipboardModule = create_default();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_exit_succeeds() {
    let mut module = ClipboardModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_dependencies_default_empty() {
    let module = ClipboardModule::new();
    assert!(module.dependencies().is_empty());
}

#[test]
fn test_init_registers_clipboard_provider() {
    use {
        reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
        std::{path::PathBuf, sync::Arc},
    };

    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = ClipboardModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify that ClipboardProviderRegistry was created in services
    let registry = services.get::<ClipboardProviderRegistry>();
    assert!(registry.is_some(), "ClipboardProviderRegistry should be registered");
}
