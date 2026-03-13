#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git signs module for reovim.
//!
//! Provides gutter annotations for git diff hunks: additions, changes,
//! and deletions. Implements `AnnotationSource` from `reovim-driver-annotation`.

use std::sync::Arc;

use {
    reovim_driver_annotation::{AnnotationSourceKey, AnnotationSourceRegistry},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

mod hunk;
mod source;

pub use {hunk::SignKind, source::GitSignsSource};

/// Git signs module.
///
/// Registers a `GitSignsSource` into the `AnnotationSourceRegistry`
/// so git diff hunks appear as gutter signs.
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

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register only the data source (display-side creates presenters)
        let source_registry = ctx.services.get_or_create::<AnnotationSourceRegistry>();
        source_registry.register(
            AnnotationSourceKey::new("git-signs"),
            Arc::new(GitSignsSource::new(ctx.services.clone())),
        );

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
