//! Compositor registry.
//!
//! Type alias for the compositor registry, keyed by purpose.

use reovim_kernel::api::v1::MultiServiceRegistry;

use super::{RootCompositor, compositor_key::CompositorKey};

/// Registry for compositors, keyed by purpose.
///
/// This is a type alias for `MultiServiceRegistry<CompositorKey, dyn RootCompositor>`.
/// Currently only `Root` key is supported for the root window compositor.
///
/// # Architecture
///
/// Following the VFS pattern (mechanism/policy separation):
/// - **Mechanism (driver)**: This registry type + `RootCompositor` trait
/// - **Policy (module)**: `HybridCompositor` in `server/modules/layout`
///
/// # Example
///
/// ```ignore
/// use reovim_driver_layout::{CompositorKey, CompositorRegistry, RootCompositor};
/// use std::sync::Arc;
///
/// // Create registry (typically done by runner)
/// let registry = CompositorRegistry::new();
///
/// // Modules register their compositors during init
/// registry.register(CompositorKey::Root, Arc::new(hybrid_compositor));
///
/// // Runner queries with typed key
/// let compositor = registry.get(&CompositorKey::Root);
/// ```
pub type CompositorRegistry = MultiServiceRegistry<CompositorKey, dyn RootCompositor>;

// Note: Tests for CompositorRegistry deferred until RootCompositor implementations
// are available from modules. The type alias itself is tested through
// MultiServiceRegistry's comprehensive test suite in the kernel.
