//! Hex dump codec.
//!
//! Formats raw bytes as a standard hex dump view. This is a one-way
//! (decode-only) codec — binary files cannot be saved back.
//!
//! Delegates to [`reovim_driver_codec_xxd::XxdCodec`] for the actual
//! hex dump formatting.

use reovim_driver_codec::{CodecError, CodecMetadata, DecodeResult};

// Re-export annotation constants from the xxd driver for backward compatibility.
pub use reovim_driver_codec_xxd::{HEX_ADDRESS_KIND, HEX_ASCII_KIND, HEX_BYTE_KIND};

/// Maximum input bytes before truncation (1 MB).
///
/// Exposed for tests. The actual truncation is handled by `XxdCodec`.
pub const MAX_HEX_INPUT_BYTES: usize = 1_048_576;

/// Hex dump codec for binary content.
///
/// Produces output in standard hex dump format:
/// ```text
/// 00000000  48 65 6c 6c 6f 20 57 6f  72 6c 64 0a 00 ff fe fd  |Hello World.....|
/// ```
///
/// This is a one-way codec: `encode()` returns `None` because binary
/// content cannot be losslessly reconstructed from the hex dump text.
pub struct HexCodec;

impl HexCodec {
    /// Create a new hex dump codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for HexCodec {
    fn default() -> Self {
        Self::new()
    }
}

// Note: HexCodec does not override views()/decode_view() — it reports
// the default ["default"] view, not XxdCodec's ["hex"]. This is correct:
// HexCodec IS the default view for binary/raw content. Multi-view codecs
// (ELF, ZIP, rlib) will use XxdCodec internally for their "hex" view.
impl reovim_driver_codec::ContentCodec for HexCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        reovim_driver_codec_xxd::XxdCodec::new().decode(raw)
    }

    fn encode(
        &self,
        _content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        None
    }
}

/// Format raw bytes as a hex dump string, returning content and annotations.
///
/// Delegates to [`reovim_driver_codec_xxd::format_xxd_dump`] with default
/// settings (16 bytes/line, 8-byte groups).
#[must_use]
pub fn format_hex_dump(raw: &[u8]) -> (String, Vec<reovim_driver_annotation::Annotation>) {
    reovim_driver_codec_xxd::format_xxd_dump(raw, 16, 8)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
