#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! UTF-8 content codec module for reovim.
//!
//! Provides UTF-8 encoding support for the content codec pipeline:
//! - BOM detection and stripping on decode, restoration on encode
//! - CRLF normalization to LF on decode, restoration on encode
//! - Round-trip fidelity: `encode(decode(bytes)) == bytes`
//!
//! # Architecture
//!
//! ```text
//! reovim-content-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-content-codec-utf8        (THIS CRATE - UTF-8 implementation)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`Utf8Codec`] into [`DefaultContentCodecRegistry`] under `ContentType::UTF8`
//! - [`Utf8Classifier`] into [`ContentClassifierStore`]

use std::sync::Arc;

use {
    reovim_content_codec::{ContentClassifierStore, ContentType},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

pub mod classifier;
pub mod codec;
pub mod domain;
pub mod factory;

pub use {
    classifier::Utf8Classifier, codec::Utf8Codec, domain::Utf8LineIndex, factory::Utf8CodecFactory,
};

/// UTF-8 content codec module.
///
/// Registers UTF-8 classifier (priority 10) and codec during init.
pub struct CodecUtf8Module {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
    content_type: Option<ContentType>,
}

impl CodecUtf8Module {
    /// Create a new UTF-8 codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registry: None,
            content_type: None,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CodecUtf8Module {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecUtf8Module {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-utf8")
    }

    fn name(&self) -> &'static str {
        "Codec UTF-8"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        let content_type = ContentType::new(ContentType::UTF8);
        registry.register(content_type.clone(), Arc::new(Utf8Codec::new()));
        self.registry = Some(registry);
        self.content_type = Some(content_type);

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(Utf8Classifier::new()));

        tracing::info!("CodecUtf8Module: registered UTF-8 codec and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        if let (Some(registry), Some(ct)) = (self.registry.take(), self.content_type.take()) {
            registry.unregister(&ct);
        }
        tracing::info!("CodecUtf8Module: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_subsys_content_codec::capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecUtf8Module);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
