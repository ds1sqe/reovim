use super::*;

#[test]
fn test_local_fs_provider_handles_file_scheme() {
    let provider = LocalFsProvider::new();
    assert!(provider.can_handle("file"));
    assert!(provider.can_handle(""));
    assert!(!provider.can_handle("mem"));
    assert!(!provider.can_handle("ssh"));
}

#[test]
fn test_local_fs_provider_creates_vfs() {
    let provider = LocalFsProvider::new();
    let vfs = provider.create("file");
    assert!(vfs.is_some());
}

#[test]
fn test_local_fs_provider_id() {
    let provider = LocalFsProvider::new();
    assert_eq!(provider.provider_id().as_str(), "vfs-local");
}

#[test]
fn test_local_fs_provider_priority() {
    let provider = LocalFsProvider::new();
    assert_eq!(provider.priority(), ProviderPriority::Default);
}

#[test]
fn test_module_provides_vfs() {
    let module = VfsLocalModule::new();
    let providers = module.vfs_providers();
    assert_eq!(providers.len(), 1);
    assert!(providers[0].can_handle("file"));
}

#[test]
fn test_module_id() {
    let module = VfsLocalModule::new();
    assert_eq!(module.id().as_str(), "vfs-local");
}

#[test]
fn test_module_name() {
    let module = VfsLocalModule::new();
    assert_eq!(module.name(), "Local Filesystem VFS");
}

#[test]
fn test_module_version() {
    let module = VfsLocalModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 1);
}

#[test]
fn test_module_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_default: VfsLocalModule = create_default();
    let from_new = VfsLocalModule::new();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_provider_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_default: LocalFsProvider = create_default();
    let from_new = LocalFsProvider::new();
    assert_eq!(from_new.provider_id(), from_default.provider_id());
}

#[test]
fn test_exit_succeeds() {
    let mut module = VfsLocalModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_provider_creates_vfs_for_empty_scheme() {
    let provider = LocalFsProvider::new();
    let vfs = provider.create("");
    assert!(vfs.is_some());
}

#[test]
fn test_provider_does_not_handle_http() {
    let provider = LocalFsProvider::new();
    assert!(!provider.can_handle("http"));
    assert!(!provider.can_handle("https"));
    assert!(!provider.can_handle("ftp"));
}

#[test]
fn test_vfs_driver_basic_ops() {
    let provider = LocalFsProvider::new();
    let vfs = provider.create("file").unwrap();

    // StandardVfs should exist as a valid VfsDriver
    // Test exists on a path that does exist
    assert!(vfs.exists(std::path::Path::new("/")));

    // Test exists on a path that does not exist
    assert!(!vfs.exists(std::path::Path::new("/nonexistent_reovim_test_path_xyz")));
}

#[test]
fn test_dependencies_default_empty() {
    let module = VfsLocalModule::new();
    assert!(module.dependencies().is_empty());
}

#[test]
fn test_init_registers_vfs_provider() {
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

    let mut module = VfsLocalModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify that VfsProviderRegistry was created in services
    let registry = services.get::<VfsProviderRegistry>();
    assert!(registry.is_some(), "VfsProviderRegistry should be registered in services");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_provider_create_returns_functional_driver() {
    let provider = LocalFsProvider::new();
    let vfs = provider.create("file").unwrap();

    // Read a known system file
    let result = vfs.read(std::path::Path::new("/proc/self/status"));
    // On Linux, this should succeed
    if let Ok(bytes) = result {
        assert!(!bytes.is_empty());
    }
}

#[test]
fn test_module_exit_is_idempotent() {
    let mut module = VfsLocalModule::new();
    assert!(module.exit().is_ok());
    assert!(module.exit().is_ok());
}

#[test]
fn test_provider_create_for_unknown_scheme_still_returns_some() {
    // create() returns Some regardless of scheme - it's can_handle() that gates
    let provider = LocalFsProvider::new();
    let vfs = provider.create("something");
    // create() always returns Some(StandardVfs::new())
    assert!(vfs.is_some());
}

#[test]
fn test_vfs_driver_read_nonexistent_file_returns_error() {
    let provider = LocalFsProvider::new();
    let vfs = provider.create("file").unwrap();
    let result = vfs.read(std::path::Path::new("/nonexistent_reovim_test_xyz_12345"));
    assert!(result.is_err());
}
