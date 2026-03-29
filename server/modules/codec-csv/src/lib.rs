#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! CSV/TSV content codec module for reovim.
//!
//! Provides delimiter-separated value file support for the content codec
//! pipeline:
//! - [`CsvClassifier`] detects CSV/TSV/PSV files by delimiter analysis
//!   or file extension (priority 15)
//! - [`CsvCodecFactory`] creates [`CsvCodec`] instances for column-aligned
//!   tabular view with round-trip editing support
//!
//! This is a BIDIRECTIONAL codec — CSV files can be edited and saved.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-csv          (THIS CRATE - CSV/TSV/PSV)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`CsvClassifier`] into [`ContentClassifierStore`] (priority 15)
//! - [`CsvCodecFactory`] into [`ContentCodecFactoryStore`]

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::CsvClassifier, codec::CsvCodec, factory::CsvCodecFactory};

/// CSV/TSV content codec module.
///
/// Registers CSV classifier (priority 15) and CSV codec factory during init.
pub struct CodecCsvModule;

impl CodecCsvModule {
    /// Create a new CSV codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CodecCsvModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecCsvModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-csv")
    }

    fn name(&self) -> &'static str {
        "Codec CSV"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(CsvCodecFactory::new()));

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(CsvClassifier::new()));

        tracing::info!("CodecCsvModule: registered CSV codec and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("CodecCsvModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecCsvModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
