//! Hex dump codec.
//!
//! Formats raw bytes as a standard hex dump view. This is a one-way
//! (decode-only) codec — binary files cannot be saved back.
//!
//! Delegates to [`reovim_driver_codec_xxd::XxdCodec`] for the actual
//! hex dump formatting.

use {
    reovim_driver_codec::{
        CodecError, ContentCodec, DecodeResult, DecodedEdit, TranslateEditError,
    },
    reovim_kernel::api::v1::ByteEdit,
};

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
impl ContentCodec for HexCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        reovim_driver_codec_xxd::XxdCodec::new().decode(raw)
    }

    fn translate_edit(
        &self,
        bytes: &dyn reovim_subsys_vfs::ByteSource,
        edit: &DecodedEdit,
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        match edit {
            DecodedEdit::Bytes {
                offset,
                old_len,
                new_bytes,
            } => {
                let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
                    reason: "hex codec: failed to read byte source",
                })?;
                let end = offset.checked_add(*old_len).ok_or(
                    TranslateEditError::ConstraintViolation {
                        reason: "hex codec: edit range overflowed",
                    },
                )?;
                let old_bytes = raw
                    .get(*offset..end)
                    .ok_or(TranslateEditError::ConstraintViolation {
                        reason: "hex codec: edit range out of bounds",
                    })?
                    .to_vec();

                Ok(Some(ByteEdit {
                    offset: *offset,
                    old_bytes,
                    new_bytes: new_bytes.clone(),
                }))
            }
            DecodedEdit::Text { .. } => Err(TranslateEditError::UnsupportedEdit {
                reason: "hex codec does not accept text edits",
            }),
            // Tree edits and any future DecodedEdit variants (#[non_exhaustive])
            // are not supported by byte-oriented codecs.
            _ => Err(TranslateEditError::UnsupportedEdit {
                reason: "hex codec only accepts byte-shaped edits",
            }),
        }
    }
}

/// Format raw bytes as a hex dump string, returning content and annotations.
///
/// Delegates to [`reovim_driver_codec_xxd::format_xxd_dump`] with default
/// settings (16 bytes/line, 8-byte groups).
#[must_use]
pub fn format_hex_dump(raw: &[u8]) -> (String, Vec<reovim_subsys_annotation::Annotation>) {
    reovim_driver_codec_xxd::format_xxd_dump(raw, 16, 8)
}

fn read_all_bytes(bytes: &dyn reovim_subsys_vfs::ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();
    (data.len() == len).then_some(data)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
