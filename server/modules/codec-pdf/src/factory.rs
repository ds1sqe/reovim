//! PDF codec factory.

use reovim_driver_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::{classifier::PDF, codec::PdfCodec};

/// Factory for creating PDF codecs.
///
/// Creates [`PdfCodec`] instances for `ContentType("document/pdf")`.
pub struct PdfCodecFactory;

impl PdfCodecFactory {
    /// Create a new PDF codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for PdfCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for PdfCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>> {
        if content_type.as_str() == PDF {
            Some(Box::new(PdfCodec::new()))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![PDF]
    }

    fn name(&self) -> &'static str {
        "pdf"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
