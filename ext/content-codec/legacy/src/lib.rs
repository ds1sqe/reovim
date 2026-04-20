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
//! reovim-content-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-content-codec-legacy      (THIS CRATE - legacy implementations)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`LegacyCodec`] instances into [`DefaultContentCodecRegistry`] for each encoding
//! - [`LegacyClassifier`] into [`ContentClassifierStore`] (priority 40)

use std::sync::Arc;

use {
    reovim_content_codec::{ContentClassifierStore, ContentType},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::LegacyClassifier, codec::LegacyCodec, factory::LegacyCodecFactory};

/// Mapping of legacy content type strings to their `is_windows_1252` flag.
const LEGACY_ENCODINGS: &[(&str, bool)] = &[
    (classifier::LATIN_1, false),
    (classifier::WINDOWS_1252, true),
];

/// Legacy encoding content codec module.
///
/// Registers legacy classifier (priority 40) and one codec per encoding during init.
pub struct CodecLegacyModule {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
    content_types: Vec<ContentType>,
}

impl CodecLegacyModule {
    /// Create a new legacy codec module.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: None,
            content_types: Vec::new(),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        let mut content_types = Vec::with_capacity(LEGACY_ENCODINGS.len());
        for &(ct_str, is_windows_1252) in LEGACY_ENCODINGS {
            let ct = ContentType::new(ct_str);
            registry.register(ct.clone(), Arc::new(LegacyCodec::new(ct_str, is_windows_1252)));
            content_types.push(ct);
        }
        self.content_types = content_types;
        self.registry = Some(registry);

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(LegacyClassifier::new()));

        tracing::info!("CodecLegacyModule: registered legacy codecs and classifier");
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
