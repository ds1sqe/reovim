//! VFS provider trait for module-driven filesystem initialization.
//!
//! This module defines the `VfsProvider` trait that modules can implement
//! to provide VFS implementations at boot time.
//!
//! # Design
//!
//! - **Single initialization path**: All VFS comes from provider registries
//! - **Panic fast**: If no VFS provider registered, system panics at boot
//! - **Priority-based**: Multiple providers can coexist, highest priority wins
//!
//! # Example
//!
//! ```ignore
//! use reovim_subsys_vfs::{VfsProvider, ProviderPriority, VfsDriver};
//! use reovim_kernel::api::v1::ModuleId;
//! use std::sync::Arc;
//!
//! struct LocalFsProvider;
//!
//! impl VfsProvider for LocalFsProvider {
//!     fn provider_id(&self) -> &ModuleId {
//!         static ID: ModuleId = ModuleId::new("runner");
//!         &ID
//!     }
//!
//!     fn can_handle(&self, scheme: &str) -> bool {
//!         scheme == "file" || scheme.is_empty()
//!     }
//!
//!     fn create(&self, _scheme: &str) -> Option<Arc<dyn VfsDriver>> {
//!         Some(Arc::new(StandardVfs::new()))
//!     }
//! }
//! ```

use std::sync::Arc;

use reovim_kernel::api::v1::ModuleId;

use crate::VfsDriver;

/// Priority for provider resolution when multiple providers exist.
///
/// Higher value = more preferred. Used only for ordering when multiple
/// providers can handle the same scheme. NOT for fallback behavior.
///
/// # Panic Fast
///
/// If no provider can handle a required scheme, the system panics.
/// There is no graceful degradation for essential providers.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ProviderPriority {
    /// Standard module provider (default).
    #[default]
    Default = 50,
    /// User configuration override (highest priority).
    Override = 100,
}

/// Trait for modules that can provide VFS implementations.
///
/// Modules implement this trait to provide VFS drivers during initialization.
/// The runner collects all providers and uses priority-based resolution.
///
/// # Panic Fast
///
/// If no VFS provider is registered at boot, the system panics immediately.
/// VFS is essential - there is no fallback or graceful degradation.
///
/// # Scheme Handling
///
/// Providers declare which URI schemes they can handle:
/// - `"file"` or `""` - local filesystem
/// - `"mem"` - in-memory filesystem (testing)
/// - `"ssh"` - remote SSH filesystem (future)
pub trait VfsProvider: Send + Sync {
    /// Module ID that provides this VFS.
    fn provider_id(&self) -> &ModuleId;

    /// Priority for this provider.
    ///
    /// When multiple providers can handle the same scheme, the one with
    /// highest priority is used.
    fn priority(&self) -> ProviderPriority {
        ProviderPriority::Default
    }

    /// Check if this provider can handle the given path scheme.
    ///
    /// # Arguments
    ///
    /// * `scheme` - URI scheme (e.g., "file", "mem", "ssh")
    ///
    /// # Returns
    ///
    /// `true` if this provider can create a VFS for the given scheme.
    fn can_handle(&self, scheme: &str) -> bool;

    /// Create a VFS driver instance for the given scheme.
    ///
    /// # Arguments
    ///
    /// * `scheme` - URI scheme that was previously accepted by `can_handle()`
    ///
    /// # Returns
    ///
    /// The VFS driver instance, or `None` if creation fails.
    fn create(&self, scheme: &str) -> Option<Arc<dyn VfsDriver>>;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
