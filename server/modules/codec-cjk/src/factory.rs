//! CJK codec factory.

use reovim_driver_codec::{ContentCodec, ContentCodecFactory, ContentType};

use crate::codec::CjkCodec;

/// Supported CJK encodings: content type string and `encoding_rs` encoding.
const CJK_ENCODINGS: &[(&str, &encoding_rs::Encoding)] = &[
    ("encoding/euc-kr", encoding_rs::EUC_KR),
    ("encoding/shift-jis", encoding_rs::SHIFT_JIS),
    ("encoding/gbk", encoding_rs::GBK),
    ("encoding/big5", encoding_rs::BIG5),
];

/// Factory for creating CJK encoding codecs.
///
/// Creates [`CjkCodec`] instances for EUC-KR, Shift-JIS, GBK, and Big5.
pub struct CjkCodecFactory;

impl CjkCodecFactory {
    /// Create a new CJK codec factory.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CjkCodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodecFactory for CjkCodecFactory {
    fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>> {
        for &(ct_str, encoding) in CJK_ENCODINGS {
            if content_type.as_str() == ct_str {
                return Some(Box::new(CjkCodec::new(encoding, ct_str)));
            }
        }
        None
    }

    fn supported_content_types(&self) -> Vec<&str> {
        CJK_ENCODINGS.iter().map(|&(ct, _)| ct).collect()
    }

    fn name(&self) -> &'static str {
        "cjk"
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
