//! CJK encoding codec.
//!
//! Decodes CJK-encoded bytes to UTF-8 and encodes back to the original
//! encoding. This is a bidirectional codec with round-trip guarantees
//! for characters supported by the encoding.

use reovim_driver_codec::{CodecError, CodecMetadata, ContentType, DecodeResult};

/// CJK encoding codec.
///
/// Uses `encoding_rs` for encoding/decoding. Stores the encoding name
/// in metadata for round-trip encoding on save.
pub struct CjkCodec {
    /// The encoding to use for decode/encode.
    encoding: &'static encoding_rs::Encoding,
    /// Content type identifier (e.g., "encoding/euc-kr").
    content_type: &'static str,
}

impl CjkCodec {
    /// Create a new CJK codec for the given encoding.
    #[must_use]
    pub const fn new(encoding: &'static encoding_rs::Encoding, content_type: &'static str) -> Self {
        Self {
            encoding,
            content_type,
        }
    }

    /// Get the encoding used by this codec.
    #[must_use]
    pub const fn encoding(&self) -> &'static encoding_rs::Encoding {
        self.encoding
    }
}

impl reovim_driver_codec::ContentCodec for CjkCodec {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let (decoded, _, had_errors) = self.encoding.decode(raw);

        if had_errors {
            return Err(CodecError::Other(format!("failed to decode as {}", self.encoding.name())));
        }

        let mut metadata = CodecMetadata::new(ContentType::new(self.content_type));
        metadata.set("encoding", self.encoding.name());

        Ok(DecodeResult {
            content: decoded.into_owned(),
            annotations: Vec::new(),
            metadata,
            lossy: false,
            readonly: false,
        })
    }

    fn encode(
        &self,
        content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        let (encoded, _, had_errors) = self.encoding.encode(content);

        if had_errors {
            return Some(Err(CodecError::Other(format!(
                "content contains characters not representable in {}",
                self.encoding.name()
            ))));
        }

        Some(Ok(encoded.into_owned()))
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
