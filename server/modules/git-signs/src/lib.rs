#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git signs module for reovim.
//!
//! Provides gutter annotations for git diff hunks: additions, changes,
//! and deletions. Implements `AnnotationSource` and `AnnotationPresenter`
//! from `reovim-driver-display`.

use std::sync::Arc;

use {
    reovim_driver_display::{GutterRendererKey, GutterRendererRegistry},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

mod hunk;
mod presenter;
mod source;

pub use {hunk::SignKind, presenter::GitSignsPresenter, source::GitSignsSource};

/// Git signs module.
///
/// Registers a `GitSignsSource` and `GitSignsPresenter` into the
/// default `GutterRenderer` so git diff hunks appear as gutter signs.
pub struct GitSignsModule;

impl GitSignsModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitSignsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GitSignsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("git-signs")
    }

    fn name(&self) -> &'static str {
        "Git Signs"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let renderer_registry = ctx.services.get_or_create::<GutterRendererRegistry>();

        if let Some(renderer) = renderer_registry.get(&GutterRendererKey::Default) {
            renderer.register_source(Arc::new(GitSignsSource::new(ctx.services.clone())));
            renderer.register_presenter(Arc::new(GitSignsPresenter::new()));
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(GitSignsModule);

#[cfg(test)]
mod tests;
