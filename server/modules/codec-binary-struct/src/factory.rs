//! Binary struct codec factory.

use reovim_driver_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::{
    classifier::{ELF, ZIP},
    codec::{ElfCodec, ZipCodec},
};

/// Factory for creating ELF and ZIP codecs.
///
/// Creates [`ElfCodec`] for `ContentType("binary/elf")` and
/// [`ZipCodec`] for `ContentType("binary/zip")`.
pub struct BinaryStructCodecFactory;

impl BinaryStructCodecFactory {
    /// Create a new binary struct codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for BinaryStructCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for BinaryStructCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>> {
        match content_type.as_str() {
            s if s == ELF => Some(Box::new(ElfCodec::new())),
            s if s == ZIP => Some(Box::new(ZipCodec::new())),
            _ => None,
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![ELF, ZIP]
    }

    fn name(&self) -> &'static str {
        "binary-struct"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
