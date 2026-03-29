#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Window layout compositor module for reovim.
//!
//! Provides `HybridCompositor`, a concrete `RootCompositor` implementation
//! with binary-split-tree tiled windows. Registers to `CompositorRegistry`
//! so bootstrap can extract and wire it into the session.

mod hybrid_compositor;
mod hybrid_layer;
pub mod tiled_zone;

use {
    hybrid_compositor::HybridCompositor,
    reovim_driver_layout::{CompositorKey, CompositorRegistry},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleId, ProbeResult, Version},
    std::sync::Arc,
};

const MODULE_ID: ModuleId = ModuleId::new("layout");

/// Layout compositor module.
///
/// Registers `HybridCompositor` into `CompositorRegistry` during `init()`.
#[derive(Debug)]
pub struct LayoutModule;

impl LayoutModule {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for LayoutModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LayoutModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "Layout"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<CompositorRegistry>();
        let compositor = HybridCompositor::new();
        registry.register(CompositorKey::Root, Arc::new(compositor));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), reovim_kernel::api::v1::ModuleError> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
