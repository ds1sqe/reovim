//! VFS instance wrapper for `ServiceRegistry`.
//!
//! This module provides a wrapper type that holds a VFS driver instance
//! and implements `Service` so it can be stored in `ServiceRegistry`.
//!
//! # Usage
//!
//! ```ignore
//! // In bootstrap:
//! let vfs: Arc<dyn VfsDriver> = Arc::new(StandardVfs::new());
//! services.register(Arc::new(VfsInstance::new(vfs)));
//!
//! // In ex-command dispatch:
//! if let Some(vfs_instance) = kernel.services.get::<VfsInstance>() {
//!     let vfs = vfs_instance.driver();
//!     ex_ctx = ex_ctx.with_vfs(Arc::clone(vfs));
//! }
//! ```

use std::sync::Arc;

use reovim_kernel::api::v1::Service;

use crate::VfsDriver;

/// Wrapper for VFS driver instance stored in `ServiceRegistry`.
///
/// This allows the VFS driver to be accessed from anywhere that has
/// access to the kernel's `ServiceRegistry`, such as ex-command dispatch.
pub struct VfsInstance {
    driver: Arc<dyn VfsDriver>,
}

impl VfsInstance {
    /// Create a new VFS instance wrapper.
    #[must_use]
    pub fn new(driver: Arc<dyn VfsDriver>) -> Self {
        Self { driver }
    }

    /// Get the underlying VFS driver.
    #[must_use]
    pub fn driver(&self) -> &Arc<dyn VfsDriver> {
        &self.driver
    }
}

// Implement Service so VfsInstance can be stored in ServiceRegistry
impl Service for VfsInstance {}

impl std::fmt::Debug for VfsInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VfsInstance").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::MockVfs};

    #[test]
    fn test_vfs_instance() {
        let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
        let instance = VfsInstance::new(Arc::clone(&vfs));

        // Should be able to get driver back
        let retrieved = instance.driver();
        assert!(Arc::ptr_eq(retrieved, &vfs));
    }

    #[test]
    fn test_vfs_instance_debug() {
        let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
        let instance = VfsInstance::new(vfs);

        let debug = format!("{instance:?}");
        assert!(debug.contains("VfsInstance"));
    }

    #[test]
    fn test_vfs_instance_driver_usable() {
        use std::path::Path;

        let mock = MockVfs::new();
        mock.add_file_str("/test.txt", "hello");
        let vfs: Arc<dyn VfsDriver> = Arc::new(mock);
        let instance = VfsInstance::new(vfs);

        let driver = instance.driver();
        let content = driver.read_to_string(Path::new("/test.txt")).unwrap();
        assert_eq!(content, "hello");
    }
}
