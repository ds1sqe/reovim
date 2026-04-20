//! CSV codec factory.

use std::sync::Arc;

use reovim_content_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::{
    classifier::{CSV, PSV, SCSV, TSV},
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

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CsvCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for CsvCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        match content_type.as_str() {
            s if s == CSV => Some(Arc::new(CsvCodec::new(b',', CSV))),
            s if s == TSV => Some(Arc::new(CsvCodec::new(b'\t', TSV))),
            s if s == PSV => Some(Arc::new(CsvCodec::new(b'|', PSV))),
            s if s == SCSV => Some(Arc::new(CsvCodec::new(b';', SCSV))),
            _ => None,
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![CSV, TSV, PSV, SCSV]
    }

    fn name(&self) -> &'static str {
        "csv"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
