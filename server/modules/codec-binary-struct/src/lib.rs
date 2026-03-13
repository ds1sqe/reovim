#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! ELF/ZIP structured binary content codec module for reovim.
//!
//! Provides structured views of ELF binaries and ZIP archives for the
//! content codec pipeline:
//! - [`ElfClassifier`] detects ELF binaries via `\x7fELF` magic (priority 33)
//! - [`ZipClassifier`] detects ZIP archives via `PK\x03\x04` magic (priority 31)
//! - [`BinaryStructCodecFactory`] creates [`ElfCodec`] or [`ZipCodec`] instances
//!
//! Both are one-way (decode-only) codecs — structured views are marked readonly.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec                    (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-binary-struct      (THIS CRATE - ELF/ZIP summaries)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`ElfClassifier`] into [`ContentClassifierStore`] (priority 33)
//! - [`ZipClassifier`] into [`ContentClassifierStore`] (priority 31)
//! - [`BinaryStructCodecFactory`] into [`ContentCodecFactoryStore`]

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {
    classifier::{ElfClassifier, ZipClassifier},
    codec::{ElfCodec, ZipCodec},
    factory::BinaryStructCodecFactory,
};

/// ELF/ZIP structured binary codec module.
///
/// Registers ELF classifier (priority 33), ZIP classifier (priority 31),
/// and the binary struct codec factory during init.
pub struct CodecBinaryStructModule;

impl CodecBinaryStructModule {
    /// Create a new binary struct codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CodecBinaryStructModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecBinaryStructModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-binary-struct")
    }

    fn name(&self) -> &'static str {
        "Codec Binary Struct"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory (handles both ELF and ZIP)
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(BinaryStructCodecFactory::new()));

        // Register classifiers
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(ElfClassifier::new()));
        classifier_store.add(Arc::new(ZipClassifier::new()));

        tracing::info!("CodecBinaryStructModule: registered ELF/ZIP codecs and classifiers");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("CodecBinaryStructModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecBinaryStructModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
