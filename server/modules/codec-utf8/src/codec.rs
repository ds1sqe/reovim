//! UTF-8 content codec.
//!
//! Handles BOM detection/stripping, CRLF normalization, and
//! round-trip encoding that preserves original BOM and line endings.

use reovim_driver_codec::{CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult};

/// UTF-8 BOM bytes.
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Metadata key for BOM presence.
pub const META_BOM: &str = "bom";

/// Metadata key for original line ending style.
pub const META_LINE_ENDING: &str = "line_ending";

/// Line ending: CRLF.
pub const LINE_ENDING_CRLF: &str = "crlf";

/// Line ending: LF (default).
pub const LINE_ENDING_LF: &str = "lf";

/// UTF-8 content codec.
///
/// Bidirectional codec that handles:
/// - BOM detection and stripping on decode, restoration on encode
/// - CRLF normalization to LF on decode, restoration on encode
/// - Clean round-trip: `encode(decode(bytes)) == bytes`
pub struct Utf8Codec;

impl Utf8Codec {
    /// Create a new UTF-8 codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for Utf8Codec {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodec for Utf8Codec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        // Check for BOM
        let (has_bom, content_bytes) = if raw.starts_with(UTF8_BOM) {
            (true, &raw[UTF8_BOM.len()..])
        } else {
            (false, raw)
        };

        // Validate UTF-8
        let text = std::str::from_utf8(content_bytes).map_err(|e| CodecError::InvalidSequence {
            offset: e.valid_up_to() + if has_bom { UTF8_BOM.len() } else { 0 },
            detail: format!("invalid UTF-8 at byte {}", e.valid_up_to()),
        })?;

        // Detect line ending and normalize CRLF to LF
        let has_crlf = text.contains("\r\n");
        let content = if has_crlf {
            text.replace("\r\n", "\n")
        } else {
            text.to_string()
        };

        // Build metadata
        let mut metadata = CodecMetadata::new(ContentType::new(ContentType::UTF8));
        if has_bom {
            metadata.set(META_BOM, "true");
        }
        if has_crlf {
            metadata.set(META_LINE_ENDING, LINE_ENDING_CRLF);
        } else {
            metadata.set(META_LINE_ENDING, LINE_ENDING_LF);
        }

        Ok(DecodeResult {
            content,
            annotations: vec![],
            metadata,
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn encode(
        &self,
        content: &str,
        metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        // Restore line endings
        let text = if metadata.get(META_LINE_ENDING) == Some(LINE_ENDING_CRLF) {
            content.replace('\n', "\r\n")
        } else {
            content.to_string()
        };

        // Restore BOM
        let mut bytes = Vec::with_capacity(text.len() + 3);
        if metadata.get(META_BOM) == Some("true") {
            bytes.extend_from_slice(UTF8_BOM);
        }
        bytes.extend_from_slice(text.as_bytes());

        Some(Ok(bytes))
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
