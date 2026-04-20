//! UTF-8 content codec.
//!
//! Handles BOM detection/stripping, CRLF normalization, and
//! round-trip encoding that preserves original BOM and line endings.

use {
    reovim_content_codec::{
        ByteNotifiable, CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult,
        DecodedEdit, Index, TranslateEditError,
    },
    reovim_content_codec_text::TextEdit,
    reovim_kernel::api::v1::ByteEdit,
};

/// UTF-8 BOM bytes.
pub(crate) const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

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

    fn translate_edit(
        &self,
        bytes: &dyn reovim_subsys_vfs::ByteSource,
        edit: &DecodedEdit,
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        match edit {
            DecodedEdit::Domain(domain_edit) => {
                let Some(text_edit) = domain_edit.downcast_ref::<TextEdit>() else {
                    return Err(TranslateEditError::UnsupportedEdit {
                        reason: "utf-8 codec only accepts text-shaped domain edits",
                    });
                };
                let start = &text_edit.start;
                let end = &text_edit.end;
                let replacement = &text_edit.replacement;
                let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
                    reason: "utf-8 codec: failed to read byte source",
                })?;
                let decoded = self
                    .decode(&raw)
                    .map_err(|_| TranslateEditError::Internal {
                        reason: "utf-8 codec: decode failed during translate_edit",
                    })?;
                let mut index = crate::domain::Utf8LineIndex::new();
                index.build(decoded.content.as_bytes());

                let start_decoded =
                    index
                        .to_bytes(start)
                        .ok_or(TranslateEditError::ConstraintViolation {
                            reason: "utf-8 codec: start position out of range",
                        })?;
                let end_decoded =
                    index
                        .to_bytes(end)
                        .ok_or(TranslateEditError::ConstraintViolation {
                            reason: "utf-8 codec: end position out of range",
                        })?;
                let start_raw = normalized_offset_to_raw_offset(&raw, start_decoded)
                    .ok_or_else(offset_start_mapping_error)?;
                let end_raw = normalized_offset_to_raw_offset(&raw, end_decoded)
                    .ok_or_else(offset_end_mapping_error)?;
                if start_raw > end_raw {
                    return Err(TranslateEditError::ConstraintViolation {
                        reason: "utf-8 codec: start offset exceeds end offset",
                    });
                }
                let old_bytes = raw
                    .get(start_raw..end_raw)
                    .ok_or(TranslateEditError::ConstraintViolation {
                        reason: "utf-8 codec: byte range slice out of bounds",
                    })?
                    .to_vec();
                Ok(Some(ByteEdit {
                    offset: start_raw,
                    old_bytes,
                    new_bytes: replacement.as_bytes().to_vec(),
                }))
            }
            DecodedEdit::Bytes { .. } => Err(TranslateEditError::UnsupportedEdit {
                reason: "utf-8 codec does not accept byte-shaped edits",
            }),
            // Tree edits and any future DecodedEdit variants (#[non_exhaustive])
            // are not supported by text-oriented codecs.
            _ => Err(TranslateEditError::UnsupportedEdit {
                reason: "utf-8 codec only accepts text-shaped edits",
            }),
        }
    }
}

/// Retained free-function helper for round-trip tests that verify
/// UTF-8 BOM / CRLF metadata contract.
///
/// Kept after `#740` Plan 06 Phase 5 sub-commit 5d deleted the
/// public `ContentCodec::encode` seam — the consolidated `:w` path
/// no longer re-encodes, but the BOM + CRLF metadata round-trip
/// logic still needs coverage.
#[cfg(test)]
pub(crate) fn encode_fragment(content: &str, metadata: &CodecMetadata) -> Vec<u8> {
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

    bytes
}

/// Error factory for a start-offset mapping failure in `translate_edit`.
///
/// `normalized_offset_to_raw_offset` only returns `None` when a decoded offset
/// falls inside a multi-byte UTF-8 sequence boundary — which cannot happen for
/// offsets produced by `Utf8LineIndex::to_bytes`, since those are always
/// character-aligned.  This path is therefore genuinely unreachable in
/// production; the helper is extracted so that `coverage(off)` applies only to
/// the dead code rather than to the surrounding logic.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn offset_start_mapping_error() -> TranslateEditError {
    TranslateEditError::ConstraintViolation {
        reason: "utf-8 codec: start offset does not map to raw bytes",
    }
}

/// Error factory for an end-offset mapping failure in `translate_edit`.
///
/// See [`offset_start_mapping_error`] for the reachability argument.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn offset_end_mapping_error() -> TranslateEditError {
    TranslateEditError::ConstraintViolation {
        reason: "utf-8 codec: end offset does not map to raw bytes",
    }
}

fn read_all_bytes(bytes: &dyn reovim_subsys_vfs::ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();

    (data.len() == len).then_some(data)
}

fn normalized_offset_to_raw_offset(raw: &[u8], normalized_offset: usize) -> Option<usize> {
    let (raw_after_bom, raw_base): (&[u8], usize) = if raw.starts_with(UTF8_BOM) {
        (&raw[UTF8_BOM.len()..], UTF8_BOM.len())
    } else {
        (raw, 0)
    };

    let mut decoded_offset = 0usize;
    let mut raw_index = 0usize;
    let body = raw_after_bom;

    let text = std::str::from_utf8(body).ok()?;

    while raw_index < body.len() {
        if decoded_offset == normalized_offset {
            return Some(raw_base + raw_index);
        }

        if text[raw_index..].starts_with("\r\n") {
            // CRLF in canonical bytes corresponds to LF in decoded text.
            if normalized_offset == decoded_offset + 1 {
                return Some(raw_base + raw_index + 2);
            }

            decoded_offset += 1;
            raw_index += 2;
            continue;
        }

        let char_len = text[raw_index..].chars().next()?.len_utf8();

        let next_decoded_offset = decoded_offset + char_len;
        if normalized_offset < next_decoded_offset {
            return None;
        }

        if normalized_offset == next_decoded_offset {
            raw_index += char_len;
            return Some(raw_base + raw_index);
        }

        decoded_offset = next_decoded_offset;
        raw_index += char_len;
    }

    if normalized_offset == decoded_offset {
        Some(raw_base + body.len())
    } else {
        None
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
