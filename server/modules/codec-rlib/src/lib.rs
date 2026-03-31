#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Rust .rlib structured binary content codec module for reovim.
//!
//! Provides structured views of Rust `.rlib` library archives:
//! - [`RlibClassifier`] detects `.rlib` files via extension + ar magic (priority 32)
//! - [`RlibCodecFactory`] creates [`RlibCodec`] instances
//!
//! The codec extracts archive member listing, rustc version, and dependency
//! information from the `.rmeta` section. One-way (decode-only) — marked readonly.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-codec                    (trait definitions + stores)
//!         ^
//!         |
//! reovim-module-codec-rlib              (THIS CRATE - .rlib summaries)
//! ```
//!
//! # Self-Registration Pattern
//!
//! During `init()`, this module registers:
//! - [`RlibClassifier`] into [`ContentClassifierStore`] (priority 32)
//! - [`RlibCodecFactory`] into [`ContentCodecFactoryStore`]

use std::sync::Arc;

use {
    reovim_driver_codec::{ContentClassifierStore, ContentCodecFactoryStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod classifier;
pub mod codec;
pub mod factory;

pub use {classifier::RlibClassifier, codec::RlibCodec, factory::RlibCodecFactory};

/// Rust .rlib structured binary codec module.
///
/// Registers rlib classifier (priority 32) and codec factory during init.
pub struct CodecRlibModule;

impl CodecRlibModule {
    /// Create a new rlib codec module.
    #[must_use]
    pub const fn new() -> Self {
        Self
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register codec factory
        let factory_store = ctx.services.get_or_create::<ContentCodecFactoryStore>();
        factory_store.add_factory(Arc::new(RlibCodecFactory::new()));

        // Register classifier
        let classifier_store = ctx.services.get_or_create::<ContentClassifierStore>();
        classifier_store.add(Arc::new(RlibClassifier::new()));

        tracing::info!("CodecRlibModule: registered rlib codec and classifier");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
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
