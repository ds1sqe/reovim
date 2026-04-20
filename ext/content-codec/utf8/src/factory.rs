//! UTF-8 codec factory.

use std::sync::Arc;

use reovim_content_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::codec::Utf8Codec;

/// Factory for creating UTF-8 codecs.
///
/// Creates [`Utf8Codec`] instances for `ContentType("text/utf-8")`.
pub struct Utf8CodecFactory;

impl Utf8CodecFactory {
    /// Create a new UTF-8 codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for Utf8CodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for Utf8CodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == ContentType::UTF8 {
            Some(Arc::new(Utf8Codec::new()))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![ContentType::UTF8]
    }

    fn name(&self) -> &'static str {
        "utf-8"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
