#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::VFS_PROVIDER]
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
#[path = "lib_tests.rs"]
mod tests;
