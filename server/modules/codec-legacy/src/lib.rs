#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Legacy encoding content codec module for reovim.
//!
//! Provides legacy Western encoding support for the content codec pipeline:
//! - Latin-1 (ISO-8859-1)
//! - Windows-1252 (CP1252)
//!
//! These are bidirectional codecs with round-trip guarantees, using
//! `encoding_rs` for Windows-1252 and direct byte mapping for Latin-1.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-legacy       (THIS CRATE - legacy implementations)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`LegacyCodecFactory`] into [`ContentCodecFactoryStore`]
//! - [`LegacyClassifier`] into [`ContentClassifierStore`] (priority 40)

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::LegacyClassifier, codec::LegacyCodec, factory::LegacyCodecFactory};

/// Legacy encoding content codec module.
///
/// Registers legacy classifier (priority 40) and factory during init.
pub struct CodecLegacyModule;

impl CodecLegacyModule {
    /// Create a new legacy codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CodecLegacyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecLegacyModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-legacy")
    }

    fn name(&self) -> &'static str {
        "Codec Legacy"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(LegacyCodecFactory::new()));

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(LegacyClassifier::new()));

        tracing::info!("CodecLegacyModule: registered legacy codecs and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("CodecLegacyModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecLegacyModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
