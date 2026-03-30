//! Legacy encoding codec.
//!
//! Decodes Latin-1 and Windows-1252 encoded bytes to UTF-8 and encodes
//! back to the original encoding. Bidirectional with round-trip guarantees.

use reovim_driver_codec::{CodecError, CodecMetadata, ContentType, DecodeResult};

/// Legacy encoding codec.
///
/// Uses `encoding_rs` for Windows-1252 encoding/decoding. For Latin-1
/// (ISO-8859-1), performs direct byte-to-Unicode mapping since Latin-1
/// maps 1:1 to Unicode code points 0x00..0xFF.
pub struct LegacyCodec {
    /// Content type identifier (e.g., "encoding/latin-1").
    content_type: &'static str,
    /// Whether this is Windows-1252 (vs Latin-1).
    is_windows_1252: bool,
}

impl LegacyCodec {
    /// Create a new legacy codec.
    #[must_use]
    pub const fn new(content_type: &'static str, is_windows_1252: bool) -> Self {
        Self {
            content_type,
            is_windows_1252,
        }
    }

    /// Check if this codec handles Windows-1252.
    #[must_use]
    pub const fn is_windows_1252(&self) -> bool {
        self.is_windows_1252
    }
}

impl reovim_driver_codec::ContentCodec for LegacyCodec {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let content = if self.is_windows_1252 {
            let (decoded, _, had_errors) = encoding_rs::WINDOWS_1252.decode(raw);
            if had_errors {
                return Err(CodecError::Other("failed to decode as Windows-1252".to_string()));
            }
            decoded.into_owned()
        } else {
            // Latin-1: each byte maps directly to its Unicode code point
            raw.iter().map(|&b| b as char).collect()
        };

        let mut metadata = CodecMetadata::new(ContentType::new(self.content_type));
        let encoding_name = if self.is_windows_1252 {
            "windows-1252"
        } else {
            "latin-1"
        };
        metadata.set("encoding", encoding_name);

        Ok(DecodeResult {
            content,
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
        if self.is_windows_1252 {
            let (encoded, _, had_errors) = encoding_rs::WINDOWS_1252.encode(content);
            if had_errors {
                return Some(Err(CodecError::Other(
                    "content contains characters not representable in Windows-1252".to_string(),
                )));
            }
            Some(Ok(encoded.into_owned()))
        } else {
            // Latin-1: check all chars are in 0x00..0xFF range
            let mut bytes = Vec::with_capacity(content.len());
            for ch in content.chars() {
                let cp = ch as u32;
                if cp > 0xFF {
                    return Some(Err(CodecError::Other(format!(
                        "character U+{cp:04X} not representable in Latin-1"
                    ))));
                }
                // Safe: cp <= 0xFF is guarded above.
                #[allow(clippy::cast_possible_truncation)]
                bytes.push(cp as u8);
            }
            Some(Ok(bytes))
        }
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
