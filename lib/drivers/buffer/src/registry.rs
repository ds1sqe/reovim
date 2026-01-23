//! Buffer manager registry.

use {
    super::BufferManagerKey,
    reovim_kernel::api::v1::{BufferManager, MultiServiceRegistry},
};

/// Registry for buffer managers, keyed by strategy.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_buffer::{BufferManagerKey, BufferManagerRegistry};
///
/// let registry = BufferManagerRegistry::new();
/// registry.register(BufferManagerKey::Simple, Arc::new(simple_manager));
///
/// let manager = registry.get(&BufferManagerKey::Simple);
/// ```
pub type BufferManagerRegistry = MultiServiceRegistry<BufferManagerKey, dyn BufferManager>;
