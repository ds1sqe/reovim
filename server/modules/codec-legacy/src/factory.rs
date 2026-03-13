//! Legacy codec factory.

use reovim_driver_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::{
    classifier::{LATIN_1, WINDOWS_1252},
    codec::LegacyCodec,
};

/// Factory for creating legacy encoding codecs.
///
/// Creates [`LegacyCodec`] instances for Latin-1 and Windows-1252.
pub struct LegacyCodecFactory;

impl LegacyCodecFactory {
    /// Create a new legacy codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LegacyCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for LegacyCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>> {
        match content_type.as_str() {
            LATIN_1 => Some(Box::new(LegacyCodec::new(LATIN_1, false))),
            WINDOWS_1252 => Some(Box::new(LegacyCodec::new(WINDOWS_1252, true))),
            _ => None,
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![LATIN_1, WINDOWS_1252]
    }

    fn name(&self) -> &'static str {
        "legacy"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
