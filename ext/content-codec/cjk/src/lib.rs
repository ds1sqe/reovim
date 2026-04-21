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
//! reovim-content-codec              (trait definitions + stores)
//!         ^
//!         |
//! reovim-content-codec-cjk         (THIS CRATE - CJK implementations)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - One [`CjkCodec`] per encoding into [`DefaultContentCodecRegistry`]
//! - [`CjkClassifier`] into [`ContentClassifierStore`] (priority 50)

use std::sync::Arc;

use {
    reovim_content_codec::{ContentClassifierStore, ContentType},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::CjkClassifier, codec::CjkCodec, factory::CjkCodecFactory};

/// CJK content type → `encoding_rs` mapping used at registration time.
const CJK_ENCODINGS: &[(&str, &encoding_rs::Encoding)] = &[
    ("encoding/euc-kr", encoding_rs::EUC_KR),
    ("encoding/shift-jis", encoding_rs::SHIFT_JIS),
    ("encoding/gbk", encoding_rs::GBK),
    ("encoding/big5", encoding_rs::BIG5),
];

/// CJK encoding content codec module.
///
/// Registers CJK classifier (priority 50) and one codec per encoding during init.
pub struct CodecCjkModule {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
    content_types: Vec<ContentType>,
}

impl CodecCjkModule {
    /// Create a new CJK codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registry: None,
            content_types: Vec::new(),
        }
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
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        let mut content_types = Vec::with_capacity(CJK_ENCODINGS.len());
        for &(ct_str, encoding) in CJK_ENCODINGS {
            let ct = ContentType::new(ct_str);
            registry.register(ct.clone(), Arc::new(CjkCodec::new(encoding, ct_str)));
            content_types.push(ct);
        }
        self.content_types = content_types;
        self.registry = Some(registry);

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(CjkClassifier::new()));

        tracing::info!("CodecCjkModule: registered CJK codecs and classifier");
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
