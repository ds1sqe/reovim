use {
    super::*,
    reovim_kernel::api::v1::{KernelContext, ServiceRegistry},
    std::{path::PathBuf, sync::Arc},
};

#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context(services: Arc<ServiceRegistry>) -> ModuleContext {
    ModuleContext::new(
        KernelContext::default(),
        services,
        PathBuf::from("/tmp"),
        PathBuf::from("/tmp"),
    )
}

#[test]
fn module_id() {
    let module = GitModule::new();
    assert_eq!(module.id().as_str(), "git");
}

#[test]
fn module_name() {
    let module = GitModule::new();
    assert_eq!(module.name(), "Git Provider");
}

#[test]
fn module_version() {
    let module = GitModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
    assert_eq!(version.patch, 0);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = GitModule::default();
    assert_eq!(module.id().as_str(), "git");
}

#[test]
fn module_exit() {
    let mut module = GitModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn module_init_registers_provider() {
    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = GitModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify provider was registered.
    let store = services.get::<GitProviderStore>();
    assert!(store.is_some());
    let provider = store.unwrap().get();
    assert!(provider.is_some());
}

#[test]
fn registered_provider_returns_data() {
    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = GitModule::new();
    module.init(&ctx);

    let store = services.get::<GitProviderStore>().unwrap();
    let git = store.get().unwrap();

    // current_branch should work on this repo (or return None if not in git)
    // Either way, it should not panic.
    let _ = git.current_branch(std::path::Path::new("."));
}
