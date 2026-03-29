//! Hex dump codec.
//!
//! Formats raw bytes as a standard hex dump view. This is a one-way
//! (decode-only) codec — binary files cannot be saved back.

use std::fmt::Write;

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_driver_codec::{CodecError, CodecMetadata, ContentType, DecodeResult},
};

/// Bytes per line in the hex dump output.
const BYTES_PER_LINE: usize = 16;

/// Annotation kind for the hex offset/address column.
pub const HEX_ADDRESS_KIND: &str = "content.hex.address";

/// Annotation kind for the hex byte columns.
pub const HEX_BYTE_KIND: &str = "content.hex.byte";

/// Annotation kind for the ASCII sidebar column.
pub const HEX_ASCII_KIND: &str = "content.hex.ascii";

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

impl reovim_driver_codec::ContentCodec for HexCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let (content, annotations) = format_hex_dump(raw);

        let mut metadata = CodecMetadata::new(ContentType::new(ContentType::BINARY_RAW));
        metadata.set("readonly", "true");

        Ok(DecodeResult {
            content,
            annotations,
            metadata,
            lossy: true,
            readonly: true,
        })
    }

    fn encode(
        &self,
        _content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        // One-way codec: binary content cannot be reconstructed from hex dump
        None
    }
}

/// Format raw bytes as a hex dump string, returning content and annotations.
///
/// Each line: `OFFSET  HH HH HH HH HH HH HH HH  HH HH HH HH HH HH HH HH  |ASCII...........|`
///
/// Emits three annotations per line:
/// - `content.hex.address` — the offset column
/// - `content.hex.byte` — the hex byte columns
/// - `content.hex.ascii` — the ASCII sidebar
fn format_hex_dump(raw: &[u8]) -> (String, Vec<Annotation>) {
    if raw.is_empty() {
        return (String::new(), Vec::new());
    }

    let line_count = raw.len().div_ceil(BYTES_PER_LINE);
    // Each line is ~78 chars + newline
    let mut output = String::with_capacity(line_count * 80);
    let mut annotations = Vec::with_capacity(line_count * 3);

    let address_kind = AnnotationKind::new(HEX_ADDRESS_KIND);
    let byte_kind = AnnotationKind::new(HEX_BYTE_KIND);
    let ascii_kind = AnnotationKind::new(HEX_ASCII_KIND);

    for (chunk_idx, chunk) in raw.chunks(BYTES_PER_LINE).enumerate() {
        let offset = chunk_idx * BYTES_PER_LINE;

        // Offset column (8 hex digits)
        let _ = write!(output, "{offset:08x}  ");

        // Hex bytes: two groups of 8
        for i in 0..BYTES_PER_LINE {
            if i < chunk.len() {
                let _ = write!(output, "{:02x} ", chunk[i]);
            } else {
                output.push_str("   ");
            }
            if i == 7 {
                output.push(' ');
            }
        }

        // ASCII sidebar
        output.push(' ');
        output.push('|');
        for &b in chunk {
            if b.is_ascii_graphic() || b == b' ' {
                output.push(b as char);
            } else {
                output.push('.');
            }
        }
        // Pad ASCII column for partial lines
        for _ in chunk.len()..BYTES_PER_LINE {
            output.push(' ');
        }
        output.push('|');
        output.push('\n');

        // Emit annotations for this line
        annotations.push(Annotation {
            kind: address_kind.clone(),
            target: AnnotationTarget::Line(chunk_idx),
            priority: 0,
            payload: AnnotationPayload::None,
        });
        annotations.push(Annotation {
            kind: byte_kind.clone(),
            target: AnnotationTarget::Line(chunk_idx),
            priority: 0,
            payload: AnnotationPayload::None,
        });
        annotations.push(Annotation {
            kind: ascii_kind.clone(),
            target: AnnotationTarget::Line(chunk_idx),
            priority: 0,
            payload: AnnotationPayload::None,
        });
    }

    (output, annotations)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
