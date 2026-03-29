#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! PDF content codec module for reovim.
//!
//! Provides PDF file support for the content codec pipeline:
//! - [`PdfClassifier`] detects PDF files via `%PDF-` magic bytes or `.pdf`
//!   extension and assigns `ContentType("document/pdf")`
//! - [`PdfCodecFactory`] creates [`PdfCodec`] instances that extract text
//!   from PDF pages with page boundary annotations
//!
//! This is a one-way (decode-only) codec — PDF buffers are marked readonly.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-pdf          (THIS CRATE - PDF text extraction)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`PdfClassifier`] into [`ContentClassifierStore`] (priority 35)
//! - [`PdfCodecFactory`] into [`ContentCodecFactoryStore`]

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::PdfClassifier, codec::PdfCodec, factory::PdfCodecFactory};

/// PDF content codec module.
///
/// Registers PDF classifier (priority 35) and PDF codec factory during init.
pub struct CodecPdfModule;

impl CodecPdfModule {
    /// Create a new PDF codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CodecPdfModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecPdfModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-pdf")
    }

    fn name(&self) -> &'static str {
        "Codec PDF"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(PdfCodecFactory::new()));

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(PdfClassifier::new()));

        tracing::info!("CodecPdfModule: registered PDF codec and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("CodecPdfModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecPdfModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
