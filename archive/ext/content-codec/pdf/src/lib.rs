#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! PDF content codec module for reovim.
//!
//! Provides PDF file support for the content codec pipeline:
//! - [`PdfClassifier`] detects PDF files via `%PDF-` magic bytes or `.pdf`
//!   extension and assigns `ContentType("document/pdf")`
//! - [`PdfCodec`] extracts text from PDF pages with page boundary annotations
//! - Structural editing supports metadata field updates via `lopdf`
//!   parse → modify → serialize (Plan 07 Phase 6)
//! - [`PdfSetMetadataCommand`] registers `:pdf-set-metadata` for
//!   command-line structural editing (Plan 07 Phase 7)
//!
//! # Architecture
//!
//! ```text
//! reovim-content-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-content-codec-pdf         (THIS CRATE - PDF text extraction + metadata editing)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`PdfClassifier`] into [`ContentClassifierStore`] (priority 35)
//! - [`PdfCodec`] into [`DefaultContentCodecRegistry`] under `"document/pdf"`
//! - [`PdfSetMetadataCommand`] into [`CommandHandlerStore`]

use std::sync::Arc;

use {
    reovim_content_codec::{ContentClassifierStore, ContentType},
    reovim_driver_command::CommandHandlerStore,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

pub mod classifier;
pub mod codec;
pub mod command;
pub mod factory;

pub use {
    classifier::PdfClassifier, codec::PdfCodec, command::PdfSetMetadataCommand,
    factory::PdfCodecFactory,
};

/// PDF content codec module.
///
/// Registers PDF classifier (priority 35) and PDF codec during init.
pub struct CodecPdfModule {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
    content_type: Option<ContentType>,
}

impl CodecPdfModule {
    /// Create a new PDF codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registry: None,
            content_type: None,
        }
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

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        let content_type = ContentType::new(classifier::PDF);
        registry.register(content_type.clone(), Arc::new(PdfCodec::new()));
        self.registry = Some(registry);
        self.content_type = Some(content_type);

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(PdfClassifier::new()));

        // Register ex-commands (Phase 7 #740: :pdf-set-metadata)
        let cmd_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in command::command_handlers() {
            cmd_store.add(handler);
        }

        tracing::info!("CodecPdfModule: registered PDF codec, classifier, and commands");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        if let (Some(registry), Some(ct)) = (self.registry.take(), self.content_type.take()) {
            registry.unregister(&ct);
        }
        tracing::info!("CodecPdfModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_subsys_content_codec::capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecPdfModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
