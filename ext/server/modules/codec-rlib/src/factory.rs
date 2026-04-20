//! Rlib codec factory.

use std::sync::Arc;

use reovim_driver_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::{classifier::RLIB, codec::RlibCodec};

/// Factory for creating rlib codecs.
///
/// Creates [`RlibCodec`] for `ContentType("binary/rlib")`.
pub struct RlibCodecFactory;

impl RlibCodecFactory {
    /// Create a new rlib codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for RlibCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for RlibCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == RLIB {
            Some(Arc::new(RlibCodec::new()))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![RLIB]
    }

    fn name(&self) -> &'static str {
        "rlib"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
