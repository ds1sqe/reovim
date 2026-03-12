use super::*;

#[test]
fn test_module_id() {
    let module = UndoModule::new();
    assert_eq!(module.id().as_str(), "undo");
}

#[test]
fn test_module_name() {
    let module = UndoModule::new();
    assert_eq!(module.name(), "Undo");
}

#[test]
fn test_module_version() {
    let module = UndoModule::new();
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
    let from_default: UndoModule = create_default();
    let from_new = UndoModule::new();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_exit_succeeds() {
    let mut module = UndoModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_dependencies_default_empty() {
    let module = UndoModule::new();
    assert!(module.dependencies().is_empty());
}

#[test]
fn test_re_exports_undo_persist_error() {
    // Verify the re-export works by using the type
    let err = UndoPersistError::Io("test error".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("test error"));
}

#[test]
fn test_re_exports_encode_decode() {
    // Verify the re-exports of encode/decode path component work
    let encoded = encode_path_component("/test/path");
    let decoded = decode_path_component(&encoded);
    assert_eq!(decoded, "/test/path");
}

#[test]
fn test_init_registers_undo_provider() {
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

    let mut module = UndoModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify that UndoProviderRegistry was created in services
    let registry = services.get::<UndoProviderRegistry>();
    assert!(registry.is_some(), "UndoProviderRegistry should be registered in services");
}
