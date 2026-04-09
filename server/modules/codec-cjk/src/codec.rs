//! CJK encoding codec.
//!
//! Decodes CJK-encoded bytes to UTF-8 and encodes back to the original
//! encoding. This is a bidirectional codec with round-trip guarantees
//! for characters supported by the encoding.

use {
    reovim_driver_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
    },
    reovim_kernel::api::v1::ByteEdit,
};

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
    codec: &CjkCodec,
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
