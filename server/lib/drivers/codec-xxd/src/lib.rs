#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone xxd-style hex dump codec.
//!
//! Provides [`XxdCodec`], a configurable hex dump implementation that can be
//! used directly by any codec module that wants a "hex" view. This is a driver
//! crate (mechanism), not a module (policy), so module-to-module dependency
//! issues are avoided.
//!
//! # Usage
//!
//! ```ignore
//! use reovim_driver_codec_xxd::XxdCodec;
//!
//! let codec = XxdCodec::new();
//! let result = codec.decode(b"\x7fELF")?;
//! ```

use std::fmt::Write;

use {
    reovim_driver_codec::{CodecError, CodecMetadata, CodecView, ContentType, DecodeResult},
    reovim_subsys_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
};

/// Annotation kind for the hex offset/address column.
pub const HEX_ADDRESS_KIND: &str = "content.hex.address";

/// Annotation kind for the hex byte columns.
pub const HEX_BYTE_KIND: &str = "content.hex.byte";

/// Annotation kind for the ASCII sidebar column.
pub const HEX_ASCII_KIND: &str = "content.hex.ascii";

/// Default bytes per line in hex dump output.
const DEFAULT_BYTES_PER_LINE: usize = 16;

/// Default group size (bytes before the mid-line gap).
const DEFAULT_GROUP_SIZE: usize = 8;

/// Default maximum input bytes before truncation (1 MB).
const DEFAULT_MAX_BYTES: usize = 1_048_576;

/// View descriptor for this codec.
const HEX_VIEW: CodecView = CodecView {
    name: "hex",
    display: "Hex Dump",
};

/// Standalone xxd-style hex dump codec.
///
/// Produces output in standard hex dump format:
/// ```text
/// 00000000  48 65 6c 6c 6f 20 57 6f  72 6c 64 0a 00 ff fe fd  |Hello World.....|
/// ```
///
/// Configurable: bytes per line, group size, and maximum input size.
/// This is a one-way codec: `encode()` returns `None`.
pub struct XxdCodec {
    bytes_per_line: usize,
    group_size: usize,
    max_bytes: usize,
}

impl XxdCodec {
    /// Create a new xxd codec with default settings.
    ///
    /// Defaults: 16 bytes/line, 8-byte groups, 1 MB max input.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes_per_line: DEFAULT_BYTES_PER_LINE,
            group_size: DEFAULT_GROUP_SIZE,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Set the number of bytes per line.
    #[must_use]
    pub const fn with_bytes_per_line(mut self, n: usize) -> Self {
        self.bytes_per_line = n;
        self
    }

    /// Set the group size (bytes before the mid-line gap).
    #[must_use]
    pub const fn with_group_size(mut self, n: usize) -> Self {
        self.group_size = n;
        self
    }

    /// Set the maximum input bytes before truncation.
    #[must_use]
    pub const fn with_max_bytes(mut self, n: usize) -> Self {
        self.max_bytes = n;
        self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for XxdCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl reovim_driver_codec::ContentCodec for XxdCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let truncated = raw.len() > self.max_bytes;
        let effective = if truncated {
            &raw[..self.max_bytes]
        } else {
            raw
        };

        let (mut content, annotations) =
            format_xxd_dump(effective, self.bytes_per_line, self.group_size);

        let mut metadata = CodecMetadata::new(ContentType::new(ContentType::BINARY_RAW));
        metadata.set("readonly", "true");

        if truncated {
            let _ = write!(
                content,
                "\n--- Truncated: showing {} of {} bytes ({} bytes omitted) ---\n",
                self.max_bytes,
                raw.len(),
                raw.len() - self.max_bytes,
            );
            metadata.set("truncated_at", self.max_bytes.to_string());
            metadata.set("total_size", raw.len().to_string());
        }

        Ok(DecodeResult {
            content,
            annotations,
            metadata,
            lossy: true,
            readonly: true,
            truncated,
        })
    }

    fn views(&self) -> &[CodecView] {
        &[HEX_VIEW]
    }

    fn decode_view(&self, raw: &[u8], view: &str) -> Result<DecodeResult, CodecError> {
        if view == "hex" {
            self.decode(raw)
        } else {
            Err(CodecError::Other(format!("unknown view: {view}")))
        }
    }
}

/// Format raw bytes as an xxd-style hex dump.
///
/// Returns the formatted string and annotations for each line.
///
/// # Arguments
///
/// * `raw` - Input bytes
/// * `bytes_per_line` - Number of bytes per output line
/// * `group_size` - Number of bytes per group (gap inserted between groups)
#[must_use]
pub fn format_xxd_dump(
    raw: &[u8],
    bytes_per_line: usize,
    group_size: usize,
) -> (String, Vec<Annotation>) {
    if raw.is_empty() {
        return (String::new(), Vec::new());
    }

    let line_count = raw.len().div_ceil(bytes_per_line);
    let mut output = String::with_capacity(line_count * (bytes_per_line * 4 + 20));
    let mut annotations = Vec::with_capacity(line_count * 3);

    let address_kind = AnnotationKind::new(HEX_ADDRESS_KIND);
    let byte_kind = AnnotationKind::new(HEX_BYTE_KIND);
    let ascii_kind = AnnotationKind::new(HEX_ASCII_KIND);

    for (chunk_idx, chunk) in raw.chunks(bytes_per_line).enumerate() {
        let offset = chunk_idx * bytes_per_line;

        // Offset column (8 hex digits)
        let _ = write!(output, "{offset:08x}  ");

        // Hex bytes with group gaps
        for i in 0..bytes_per_line {
            if i < chunk.len() {
                let _ = write!(output, "{:02x} ", chunk[i]);
            } else {
                output.push_str("   ");
            }
            if group_size > 0 && i + 1 < bytes_per_line && (i + 1) % group_size == 0 {
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
        for _ in chunk.len()..bytes_per_line {
            output.push(' ');
        }
        output.push('|');
        output.push('\n');

        // Annotations
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
#[path = "lib_tests.rs"]
mod tests;
