//! tar.gz codec factory.

use std::sync::Arc;

use reovim_driver_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::{classifier::TAR_GZ, codec::TarGzCodec};

/// Factory for creating tar.gz codecs.
///
/// Creates [`TarGzCodec`] for `ContentType("binary/tar-gz")`.
pub struct TarGzCodecFactory;

impl TarGzCodecFactory {
    /// Create a new tar.gz codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for TarGzCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for TarGzCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == TAR_GZ {
            Some(Arc::new(TarGzCodec::new()))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![TAR_GZ]
    }

    fn name(&self) -> &'static str {
        "tar-gz"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
