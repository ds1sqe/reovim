//! Compositor key - typed key for compositor lookup.
//!
//! Defines the typed key enum for compositor discovery.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for compositor lookup.
///
/// This enum defines the purposes for which compositors can be registered.
/// Currently only `Root` is supported for the root window compositor.
///
/// # Compile-Time Safety
///
/// Using typed keys instead of strings ensures:
/// - Typos are caught at compile time (`CompositorKey::Rot` → error)
/// - Exhaustive matching in `match` statements
/// - Self-documenting API
///
/// # Example
///
/// ```ignore
/// use reovim_driver_layout::{CompositorKey, CompositorRegistry, RootCompositor};
/// use std::sync::Arc;
///
/// let registry = CompositorRegistry::new();
/// registry.register(CompositorKey::Root, Arc::new(HybridCompositor::new()));
///
/// // Lookup with typed key
/// let compositor = registry.get(&CompositorKey::Root);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositorKey {
    /// Root compositor for the display tree.
    ///
    /// This is the top-level compositor that manages all window layers.
    /// Implemented by `HybridCompositor` in `server/modules/layout`.
    Root,
}

impl ServiceKey for CompositorKey {
    fn service_name() -> &'static str {
        "Compositor"
    }
}

#[cfg(test)]
#[path = "compositor_key_tests.rs"]
mod tests;
