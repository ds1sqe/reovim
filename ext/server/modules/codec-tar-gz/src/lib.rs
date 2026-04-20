#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! tar.gz structured binary content codec module for reovim.
//!
//! Provides structured views of `.tar.gz` and `.tgz` archives:
//! - [`TarGzClassifier`] detects tar.gz files via extension + gzip magic (priority 30)
//! - [`TarGzCodecFactory`] creates [`TarGzCodec`] instances
//!
//! The codec decompresses the gzip stream, parses the tar archive, and
//! produces a member listing. Structural edits decompress, patch, and
//! recompress the full archive.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec                    (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-tar-gz            (THIS CRATE - tar.gz summaries)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`TarGzClassifier`] into [`ContentClassifierStore`] (priority 30)
//! - [`TarGzCodecFactory`] into [`ContentCodecFactoryStore`]

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::TarGzClassifier, codec::TarGzCodec, factory::TarGzCodecFactory};

/// tar.gz structured binary codec module.
///
/// Registers tar.gz classifier (priority 30) and codec factory during init.
pub struct CodecTarGzModule;

impl CodecTarGzModule {
    /// Create a new tar.gz codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CodecTarGzModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecTarGzModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-tar-gz")
    }

    fn name(&self) -> &'static str {
        "Codec Tar-Gz"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(TarGzCodecFactory::new()));

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(TarGzClassifier::new()));

        tracing::info!("CodecTarGzModule: registered tar.gz codec and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("CodecTarGzModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecTarGzModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
