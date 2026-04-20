//! Hex codec factory.

use std::sync::Arc;

use reovim_content_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::codec::HexCodec;

/// Factory for creating hex dump codecs.
///
/// Creates [`HexCodec`] instances for `ContentType::BINARY_RAW`.
pub struct HexCodecFactory;

impl HexCodecFactory {
    /// Create a new hex codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for HexCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for HexCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == ContentType::BINARY_RAW {
            Some(Arc::new(HexCodec::new()))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![ContentType::BINARY_RAW]
    }

    fn name(&self) -> &'static str {
        "hex"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
