#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git blame module for reovim.
//!
//! Provides gutter annotations for git blame information: hash, author,
//! and commit summary per line. Implements `AnnotationSource` and
//! `AnnotationPresenter` from `reovim-driver-display`.

use std::sync::Arc;

use {
    reovim_driver_display::{GutterRendererKey, GutterRendererRegistry},
    reovim_driver_git::GitProviderStore,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

mod format;
mod presenter;
mod source;

pub use {presenter::BlamePresenter, source::BlameAnnotationSource};

/// Git blame module.
///
/// Registers a `BlameAnnotationSource` and `BlamePresenter` into the
/// default `GutterRenderer` so git blame info appears in the gutter.
pub struct GitBlameModule;

impl GitBlameModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitBlameModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GitBlameModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("git-blame")
    }

    fn name(&self) -> &'static str {
        "Git Blame"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let renderer_registry = ctx.services.get_or_create::<GutterRendererRegistry>();

        if let Some(renderer) = renderer_registry.get(&GutterRendererKey::Default) {
            let store = ctx.services.get_or_create::<GitProviderStore>();
            if let Some(provider) = store.get() {
                let source = BlameAnnotationSource::new(provider);
                renderer.register_source(Arc::new(source));
                renderer.register_presenter(Arc::new(BlamePresenter::new()));
            }
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(GitBlameModule);

#[cfg(test)]
mod tests;
