#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! CJK encoding content codec module for reovim.
//!
//! Provides CJK encoding support for the content codec pipeline:
//! - EUC-KR (Korean)
//! - Shift-JIS (Japanese)
//! - GBK (Simplified Chinese)
//! - Big5 (Traditional Chinese)
//!
//! These are bidirectional codecs with round-trip guarantees for characters
//! supported by each encoding, using `encoding_rs` for the actual work.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-cjk          (THIS CRATE - CJK implementations)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`CjkCodecFactory`] into [`ContentCodecFactoryStore`]
//! - [`CjkClassifier`] into [`ContentClassifierStore`] (priority 50)

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::CjkClassifier, codec::CjkCodec, factory::CjkCodecFactory};

/// CJK encoding content codec module.
///
/// Registers CJK classifier (priority 50) and factory during init.
pub struct CodecCjkModule;

impl CodecCjkModule {
    /// Create a new CJK codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CodecCjkModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecCjkModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-cjk")
    }

    fn name(&self) -> &'static str {
        "Codec CJK"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(CjkCodecFactory::new()));

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(CjkClassifier::new()));

        tracing::info!("CodecCjkModule: registered CJK codecs and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("CodecCjkModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecCjkModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
