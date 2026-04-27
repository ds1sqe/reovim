//! VFS provider registry.
//!
//! Type alias for the VFS provider registry, keyed by scheme.

use reovim_kernel::api::v1::MultiServiceRegistry;

use crate::{VfsDriver, scheme::VfsScheme};

/// Registry for VFS providers, keyed by scheme.
///
/// This is a type alias for `MultiServiceRegistry<VfsScheme, dyn VfsDriver>`.
/// Each scheme (file, mem, ssh) maps to a specific VFS driver implementation.
///
/// # Architecture
///
/// Following the VFS pattern (mechanism/policy separation):
/// - **Mechanism (driver)**: This registry type + `VfsDriver` trait
/// - **Policy (module)**: `LocalFsProvider` in `ext/server/modules/vfs-local`
///
/// # Example
///
/// ```ignore
/// use reovim_subsys_vfs::{VfsScheme, VfsProviderRegistry, VfsDriver};
/// use std::sync::Arc;
///
/// // Create registry (typically done by runner)
/// let registry = VfsProviderRegistry::new();
///
/// // Modules register their providers during init
/// registry.register(VfsScheme::File, Arc::new(local_fs_provider));
///
/// // Runner queries with typed key
/// let driver = registry.get(&VfsScheme::File);
/// ```
pub type VfsProviderRegistry = MultiServiceRegistry<VfsScheme, dyn VfsDriver>;

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
