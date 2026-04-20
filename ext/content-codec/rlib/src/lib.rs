#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Rust .rlib structured binary content codec module for reovim.
//!
//! Provides structured views of Rust `.rlib` library archives:
//! - [`RlibClassifier`] detects `.rlib` files via extension + ar magic (priority 32)
//! - [`RlibCodec`] extracts archive member listing, rustc version, and dependency info
//!
//! The codec is one-way (decode-only) — marked readonly.
//!
//! # Architecture
//!
//! ```text
//! reovim-content-codec                    (trait definitions + stores)
//!         ^
//!         |
//! reovim-content-codec-rlib             (THIS CRATE - .rlib summaries)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`RlibClassifier`] into [`ContentClassifierStore`] (priority 32)
//! - [`RlibCodec`] into [`DefaultContentCodecRegistry`] under `"binary/rlib"`

use std::sync::Arc;

use {
    reovim_content_codec::{ContentClassifierStore, ContentType},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::RlibClassifier, codec::RlibCodec, factory::RlibCodecFactory};

/// Rust .rlib structured binary codec module.
///
/// Registers rlib classifier (priority 32) and codec during init.
pub struct CodecRlibModule {
    registry: Option<Arc<DefaultContentCodecRegistry>>,
    content_type: Option<ContentType>,
}

impl CodecRlibModule {
    /// Create a new rlib codec module.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: None,
            content_type: None,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CodecRlibModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CodecRlibModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("codec-rlib")
    }

    fn name(&self) -> &'static str {
        "Codec Rlib"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
        let content_type = ContentType::new(classifier::RLIB);
        registry.register(content_type.clone(), Arc::new(RlibCodec::new()));
        self.registry = Some(registry);
        self.content_type = Some(content_type);

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(RlibClassifier::new()));

        tracing::info!("CodecRlibModule: registered rlib codec and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        if let (Some(registry), Some(ct)) = (self.registry.take(), self.content_type.take()) {
            registry.unregister(&ct);
        }
        tracing::info!("CodecRlibModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::CODEC_PROVIDER]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CodecRlibModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
