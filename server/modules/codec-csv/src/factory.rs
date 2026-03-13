//! CSV codec factory.

use reovim_driver_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::{
    classifier::{CSV, PSV, TSV},
    codec::CsvCodec,
};

/// Factory for creating CSV/TSV/PSV codecs.
///
/// Creates [`CsvCodec`] instances with the appropriate delimiter for
/// each content type.
pub struct CsvCodecFactory;

impl CsvCodecFactory {
    /// Create a new CSV codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CsvCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for CsvCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>> {
        match content_type.as_str() {
            s if s == CSV => Some(Box::new(CsvCodec::new(b',', CSV))),
            s if s == TSV => Some(Box::new(CsvCodec::new(b'\t', TSV))),
            s if s == PSV => Some(Box::new(CsvCodec::new(b'|', PSV))),
            _ => None,
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![CSV, TSV, PSV]
    }

    fn name(&self) -> &'static str {
        "csv"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
