//! Legacy encoding codec.
//!
//! Decodes Latin-1 and Windows-1252 encoded bytes to UTF-8 and encodes
//! back to the original encoding. Bidirectional with round-trip guarantees.

use {
    reovim_driver_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
    },
    reovim_kernel::api::v1::ByteEdit,
};

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
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        bytes: &dyn reovim_driver_vfs::ByteSource,
        edit: &DecodedEdit,
    ) -> Option<ByteEdit> {
        let raw = read_all_bytes(bytes)?;
        let decoded = self.decode(&raw).ok()?;

        let (start, end, replacement) = match edit {
            DecodedEdit::Text {
                start,
                end,
                replacement,
            } => (start, end, replacement.as_str()),
            _ => return None,
        };

        let start_decoded = text_position_to_offset(&decoded.content, start)?;
        let end_decoded = text_position_to_offset(&decoded.content, end)?;
        if end_decoded < start_decoded {
            return None;
        }

        let start_raw =
            encode_prefix_len(&decoded.content, start_decoded, &decoded.metadata, self)?;
        let end_raw = encode_prefix_len(&decoded.content, end_decoded, &decoded.metadata, self)?;

        let old_bytes = raw.get(start_raw..end_raw)?.to_vec();
        let Some(Ok(new_bytes)) = self.encode(replacement, &decoded.metadata) else {
            return None;
        };

        Some(ByteEdit {
            offset: start_raw,
            old_bytes,
            new_bytes,
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

fn text_position_to_offset(text: &str, pos: &reovim_types_text::Position) -> Option<usize> {
    let mut line_start = 0usize;

    for _ in 0..pos.line {
        let rest = text.get(line_start..)?;
        let next_newline = rest.find('\n')?;
        line_start += next_newline + 1;
    }

    let line_end = text
        .get(line_start..)
        .and_then(|tail| tail.find('\n').map(|idx| idx + line_start))
        .unwrap_or(text.len());

    let line = text.get(line_start..line_end)?;

    if pos.column == 0 {
        return Some(line_start);
    }

    let mut chars_seen = 0usize;
    for (offset, _) in line.char_indices() {
        if chars_seen == pos.column {
            return Some(line_start + offset);
        }
        chars_seen += 1;
    }

    (chars_seen == pos.column).then_some(line_end)
}

fn encode_prefix_len(
    decoded: &str,
    prefix_len: usize,
    metadata: &CodecMetadata,
    codec: &LegacyCodec,
) -> Option<usize> {
    let prefix = decoded.get(0..prefix_len)?;
    match codec.encode(prefix, metadata) {
        Some(Ok(bytes)) => Some(bytes.len()),
        _ => None,
    }
}

fn read_all_bytes(bytes: &dyn reovim_driver_vfs::ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();
    (data.len() == len).then_some(data)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
