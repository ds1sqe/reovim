#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Binary hex-dump content codec module for reovim.
//!
//! Provides binary file support for the content codec pipeline:
//! - [`BinaryClassifier`] detects binary files (null bytes, non-printable ratio,
//!   known extensions) and assigns `ContentType::BINARY_RAW`
//! - [`HexCodecFactory`] creates [`HexCodec`] instances that decode raw bytes
//!   into standard hex dump format
//!
//! This is a one-way (decode-only) codec — binary buffers are marked readonly.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-hex          (THIS CRATE - binary/hex implementation)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`BinaryClassifier`] into [`ContentClassifierStore`] (priority 20)
//! - [`HexCodecFactory`] into [`ContentCodecFactoryStore`]

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::BinaryClassifier, codec::HexCodec, factory::HexCodecFactory};

/// Binary hex-dump content codec module.
///
/// Registers binary classifier (priority 20) and hex codec factory during init.
pub struct CodecHexModule;

impl CodecHexModule {
    /// Create a new hex codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CodecHexModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecHexModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-hex")
    }

    fn name(&self) -> &'static str {
        "Codec Hex"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(HexCodecFactory::new()));

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(BinaryClassifier::new()));

        tracing::info!("CodecHexModule: registered hex codec and binary classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("CodecHexModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecHexModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
