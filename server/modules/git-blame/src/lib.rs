#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git blame module for reovim.
//!
//! Provides gutter annotations for git blame information: hash, author,
//! and commit summary per line. Implements `AnnotationSource` from
//! `reovim-driver-annotation`.

use std::sync::Arc;

use {
    reovim_driver_annotation::{AnnotationSourceKey, AnnotationSourceRegistry},
    reovim_driver_git::GitProviderStore,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

mod format;
mod source;

pub use source::BlameAnnotationSource;

/// Git blame module.
///
/// Registers a `BlameAnnotationSource` into the `AnnotationSourceRegistry`
/// so git blame info appears in the gutter.
pub struct GitBlameModule;

impl GitBlameModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register only the data source (display-side creates presenters)
        let store = ctx.services.get_or_create::<GitProviderStore>();
        if let Some(provider) = store.get() {
            let source = BlameAnnotationSource::new(provider);
            let source_registry = ctx.services.get_or_create::<AnnotationSourceRegistry>();
            source_registry.register(AnnotationSourceKey::new("git-blame"), Arc::new(source));
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
