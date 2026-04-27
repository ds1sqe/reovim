#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! CSV/TSV content codec module for reovim.
//!
//! Provides delimiter-separated value file support for the content codec
//! pipeline:
//! - [`CsvClassifier`] detects CSV/TSV/PSV files by delimiter analysis
//!   or file extension (priority 15)
//! - [`CsvCodec`] provides column-aligned tabular view with round-trip editing
//!
//! This is a BIDIRECTIONAL codec — CSV files can be edited and saved.
//!
//! # Architecture
//!
//! ```text
//! reovim-content-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-content-codec-csv         (THIS CRATE - CSV/TSV/PSV)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`CsvClassifier`] into [`ContentClassifierStore`] (priority 15)
//! - One [`CsvCodec`] per delimiter into [`DefaultContentCodecRegistry`]

use std::sync::Arc;

use {
    reovim_content_codec::{ContentClassifierStore, ContentType},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::CsvClassifier, codec::CsvCodec, factory::CsvCodecFactory};

/// Mapping of CSV content type strings to their delimiter bytes.
const CSV_TYPE_DELIMITERS: &[(&str, u8)] = &[
    (classifier::CSV, b','),
    (classifier::TSV, b'\t'),
    (classifier::PSV, b'|'),
    (classifier::SCSV, b';'),
];

/// CSV/TSV content codec module.
///
/// Registers CSV classifier (priority 15) and one codec per delimiter during init.
pub struct CodecCsvModule {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
    content_types: Vec<ContentType>,
}

impl CodecCsvModule {
    /// Create a new CSV codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registry: None,
            content_types: Vec::new(),
        }
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
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        let mut content_types = Vec::with_capacity(CSV_TYPE_DELIMITERS.len());
        for &(ct_str, delimiter) in CSV_TYPE_DELIMITERS {
            let ct = ContentType::new(ct_str);
            registry.register(ct.clone(), Arc::new(CsvCodec::new(delimiter, ct_str)));
            content_types.push(ct);
        }
        self.content_types = content_types;
        self.registry = Some(registry);

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(CsvClassifier::new()));

        tracing::info!("CodecCsvModule: registered CSV codec and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        if let Some(registry) = self.registry.take() {
            for ct in self.content_types.drain(..) {
                registry.unregister(&ct);
            }
        } else {
            self.content_types.clear();
        }
        tracing::info!("CodecCsvModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_subsys_content_codec::capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecCsvModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
