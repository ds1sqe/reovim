//! Domain-generic codec implementations for the text domain.
//!
//! Implements [`Decode<Text>`], [`Encode<Text>`], and [`Index<Text>`]
//! for [`Utf8Codec`], bridging the existing content codec with the
//! new domain-aware trait system.

use {
    reovim_domain_text::{Text, TextEdit, TextPosition},
    reovim_content_codec::{
        ByteNotifiable, CodecError, CodecMetadata, ContentType, Decode, DecodeOutput, Encode, Index,
    },
    reovim_kernel::api::v1::ByteEdit,
};

use crate::codec::{LINE_ENDING_CRLF, LINE_ENDING_LF, META_BOM, META_LINE_ENDING, UTF8_BOM};

use super::Utf8Codec;

// ─── Decode<Text> ───────────────────────────────────────────────────────────

impl Decode<Text> for Utf8Codec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeOutput<Text>, CodecError> {
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
        metadata.set(
            META_LINE_ENDING,
            if has_crlf {
                LINE_ENDING_CRLF
            } else {
                LINE_ENDING_LF
            },
        );

        Ok(DecodeOutput {
            content,
            metadata,
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

// ─── Encode<Text> ───────────────────────────────────────────────────────────

impl Encode<Text> for Utf8Codec {
    fn encode(&self, content: &String, metadata: &CodecMetadata) -> Result<Vec<u8>, CodecError> {
        // Restore line endings
        let text = if metadata.get(META_LINE_ENDING) == Some(LINE_ENDING_CRLF) {
            content.replace('\n', "\r\n")
        } else {
            content.clone()
        };

        // Restore BOM
        let mut bytes = Vec::with_capacity(text.len() + 3);
        if metadata.get(META_BOM) == Some("true") {
            bytes.extend_from_slice(UTF8_BOM);
        }
        bytes.extend_from_slice(text.as_bytes());

        Ok(bytes)
    }
}

// ─── Index<Text> ────────────────────────────────────────────────────────────

/// UTF-8 line index for position <-> byte offset mapping.
///
/// Stores line start byte offsets for O(log n) position lookups and
/// maintains a copy of the raw bytes for proper multi-byte UTF-8
/// char/byte translation. Updated incrementally on byte edits.
pub struct Utf8LineIndex {
    /// Byte offset of the start of each line. `line_starts[0]` is always 0.
    line_starts: Vec<usize>,
    /// Total byte length of indexed content.
    total_bytes: usize,
    /// Raw content bytes for proper UTF-8 char↔byte translation.
    ///
    /// Maintained in sync with `line_starts` via [`ByteNotifiable::build`]
    /// and [`ByteNotifiable::notify`]. Required because `TextPosition.column`
    /// counts Unicode scalar values (chars), not bytes — a direct byte offset
    /// only works for ASCII content.
    raw: Vec<u8>,
}

impl Utf8LineIndex {
    /// Create an empty index.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            line_starts: Vec::new(),
            total_bytes: 0,
            raw: Vec::new(),
        }
    }
}

impl Default for Utf8LineIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Count occurrences of a byte in a slice.
#[allow(clippy::naive_bytecount)] // Small edits don't benefit from SIMD
fn count_byte(bytes: &[u8], target: u8) -> usize {
    bytes.iter().filter(|&&b| b == target).count()
}

impl ByteNotifiable for Utf8LineIndex {
    fn build(&mut self, raw: &[u8]) {
        self.raw = raw.to_vec();
        self.line_starts.clear();
        self.line_starts.push(0);
        for (i, &b) in raw.iter().enumerate() {
            if b == b'\n' {
                self.line_starts.push(i + 1);
            }
        }
        self.total_bytes = raw.len();
    }

    fn notify(&mut self, edit: &ByteEdit) {
        let offset = edit.offset;
        let old_len = edit.old_bytes.len();
        let new_len = edit.new_bytes.len();

        // Apply edit to raw bytes so to_bytes/offset_to_position stay accurate.
        let end = (offset + old_len).min(self.raw.len());
        self.raw.splice(offset..end, edit.new_bytes.iter().copied());

        // Count newlines removed and added
        let old_newlines = count_byte(&edit.old_bytes, b'\n');
        let new_newlines = count_byte(&edit.new_bytes, b'\n');

        // Find the line containing the edit offset
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        // Remove line starts that fall within the deleted range
        if old_newlines > 0 {
            let remove_start = line_idx + 1;
            let remove_end = (remove_start + old_newlines).min(self.line_starts.len());
            if remove_start < remove_end {
                self.line_starts.drain(remove_start..remove_end);
            }
        }

        // Adjust offsets of all subsequent lines
        if new_len != old_len {
            for start in &mut self.line_starts[(line_idx + 1)..] {
                if new_len >= old_len {
                    *start += new_len - old_len;
                } else {
                    *start -= old_len - new_len;
                }
            }
        }

        // Insert new line starts for newlines in the new bytes
        if new_newlines > 0 {
            let mut insert_pos = line_idx + 1;
            for (i, &b) in edit.new_bytes.iter().enumerate() {
                if b == b'\n' {
                    let new_line_start = offset + i + 1;
                    self.line_starts.insert(insert_pos, new_line_start);
                    insert_pos += 1;
                }
            }
        }

        if new_len >= old_len {
            self.total_bytes += new_len - old_len;
        } else {
            self.total_bytes -= old_len - new_len;
        }
    }
}

impl Index<Text> for Utf8LineIndex {
    fn to_bytes(&self, pos: &TextPosition) -> Option<usize> {
        let line_start = *self.line_starts.get(pos.line)?;
        let line_end = self
            .line_starts
            .get(pos.line + 1)
            .copied()
            .unwrap_or(self.total_bytes);

        if pos.column == 0 {
            return Some(line_start);
        }

        // Walk UTF-8 chars to find the byte offset of char `pos.column`.
        let line_bytes = self.raw.get(line_start..line_end)?;
        let line_str = std::str::from_utf8(line_bytes).ok()?;

        let mut char_count = 0;
        for (byte_idx, _) in line_str.char_indices() {
            if char_count == pos.column {
                return Some(line_start + byte_idx);
            }
            char_count += 1;
        }
        // Column equals total char count → position just past last char.
        if char_count == pos.column {
            Some(line_end)
        } else {
            None
        }
    }

    fn offset_to_position(&self, offset: usize) -> Option<TextPosition> {
        if self.line_starts.is_empty() || offset > self.total_bytes {
            return None;
        }

        // Binary search for the line containing this offset.
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        let line_start = self.line_starts[line];

        if offset == line_start {
            return Some(TextPosition::new(line, 0));
        }

        // Count UTF-8 chars from line start to offset.
        let bytes_in_line = self.raw.get(line_start..offset)?;
        let line_str = std::str::from_utf8(bytes_in_line).ok()?;
        let column = line_str.chars().count();

        Some(TextPosition::new(line, column))
    }

    fn translate_edit(&self, edit: &TextEdit) -> Option<ByteEdit> {
        match edit {
            TextEdit::Insert { position, text } => {
                let offset = self.to_bytes(position)?;
                Some(ByteEdit::insert(offset, text.as_bytes()))
            }
            TextEdit::Delete { position, text } => {
                let offset = self.to_bytes(position)?;
                Some(ByteEdit::delete(offset, text.as_bytes()))
            }
        }
    }
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod tests;
