//! Local filesystem VFS provider module.
//!
//! This module provides the [`LocalFsProvider`] which handles the `file://` scheme
//! using the standard filesystem.
//!
//! # Architecture
//!
//! Follows the mechanism/policy separation:
//! - **Mechanism**: `VfsProvider` trait (in `reovim-driver-vfs`)
//! - **Policy**: `LocalFsProvider` (this module) provides local filesystem access
//!
//! # Future Extensions
//!
//! Other VFS modules can be created for different schemes:
//! - `vfs-mem` - In-memory filesystem for testing
//! - `vfs-ssh` - Remote filesystem via SSH
//! - `vfs-archive` - Read-only access to archives (zip, tar)

use std::sync::Arc;

use {
    reovim_driver_vfs::{
        ProviderPriority, StandardVfs, VfsDriver, VfsProvider, VfsProviderModule,
        VfsProviderRegistry, VfsScheme,
    },
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

// ============================================================================
// LocalFsProvider - VFS provider implementation
// ============================================================================

/// Local filesystem VFS provider.
///
/// Provides VFS access to the local filesystem using `StandardVfs`.
/// Registered with `Default` priority.
///
/// # Schemes
///
/// - `"file"` - explicit file:// scheme
/// - `""` (empty) - default scheme for local paths
pub struct LocalFsProvider;

impl LocalFsProvider {
    /// Create a new local filesystem provider.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LocalFsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl VfsProvider for LocalFsProvider {
    fn provider_id(&self) -> &ModuleId {
        static ID: ModuleId = ModuleId::new("vfs-local");
        &ID
    }

    fn priority(&self) -> ProviderPriority {
        ProviderPriority::Default
    }

    fn can_handle(&self, scheme: &str) -> bool {
        // Handle file:// scheme and empty scheme (default for local paths)
        scheme == "file" || scheme.is_empty()
    }

    fn create(&self, _scheme: &str) -> Option<Arc<dyn VfsDriver>> {
        Some(Arc::new(StandardVfs::new()))
    }
}

// ============================================================================
// Module trait implementation
// ============================================================================

/// Local filesystem VFS module.
///
/// Provides the `LocalFsProvider` for local file access.
pub struct VfsLocalModule;

impl VfsLocalModule {
    /// Create a new VFS local module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VfsLocalModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VfsLocalModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("vfs-local")
    }

    fn name(&self) -> &'static str {
        "Local Filesystem VFS"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 1)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register VFS provider with typed key (Epic #417)
        let vfs_registry = ctx.services.get_or_create::<VfsProviderRegistry>();
        vfs_registry.register(VfsScheme::File, Arc::new(StandardVfs::new()));

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl VfsProviderModule for VfsLocalModule {
    fn vfs_providers(&self) -> Vec<Arc<dyn VfsProvider>> {
        vec![Arc::new(LocalFsProvider::new())]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VfsLocalModule);

#[cfg(test)]
mod tests {
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
}
