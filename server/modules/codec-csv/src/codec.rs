//! CSV/TSV/PSV codec.
//!
//! Decodes delimiter-separated value files into column-aligned tabular
//! text with annotations. This is a BIDIRECTIONAL codec — edited text
//! can be re-encoded back to the original delimiter format.

use std::fmt::Write;

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_driver_codec::{
        CodecError, CodecMetadata, ContentType, DecodeResult, DecodedEdit, TranslateEditError,
    },
    reovim_kernel::api::v1::ByteEdit,
};

/// Annotation kind for the header row.
pub const CSV_HEADER_KIND: &str = "content.csv.header";

/// Annotation kind for column boundaries.
pub const CSV_COLUMN_KIND: &str = "content.csv.column";

/// CSV/TSV/PSV codec.
///
/// Decodes raw bytes into a column-aligned table view. On encode,
/// reconstructs the original delimiter format from metadata.
pub struct CsvCodec {
    /// The delimiter character for this codec instance.
    delimiter: u8,
    /// The content type string.
    content_type: &'static str,
}

impl CsvCodec {
    /// Create a new CSV codec with the given delimiter.
    #[must_use]
    pub const fn new(delimiter: u8, content_type: &'static str) -> Self {
        Self {
            delimiter,
            content_type,
        }
    }

    /// Get the delimiter used by this codec.
    #[must_use]
    pub const fn delimiter(&self) -> u8 {
        self.delimiter
    }
}

impl reovim_driver_codec::ContentCodec for CsvCodec {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let text = std::str::from_utf8(raw)
            .map_err(|e| CodecError::Other(format!("CSV decode: invalid UTF-8: {e}")))?;

        let (rows, has_header) = parse_csv(text, self.delimiter);
        let (content, annotations) = format_table(&rows, has_header);

        let mut metadata = CodecMetadata::new(ContentType::new(self.content_type));
        metadata.set("delimiter", String::from(self.delimiter as char));
        metadata.set("has_header", has_header.to_string());

        // Detect line ending style
        let line_ending = if text.contains("\r\n") { "crlf" } else { "lf" };
        metadata.set("line_ending", line_ending);

        Ok(DecodeResult {
            content,
            annotations,
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
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        let (start, end, replacement) = match edit {
            DecodedEdit::Text {
                start,
                end,
                replacement,
            } => (start, end, replacement.as_str()),
            DecodedEdit::Bytes { .. } => {
                return Err(TranslateEditError::UnsupportedEdit {
                    reason: "csv codec does not accept byte-shaped edits",
                });
            }
            DecodedEdit::Tree { .. } => {
                return Err(TranslateEditError::UnsupportedEdit {
                    reason: "csv codec does not accept tree-shaped edits",
                });
            }
            _ => {
                return Err(TranslateEditError::UnsupportedEdit {
                    reason: "csv codec received an unknown decoded edit variant",
                });
            }
        };

        let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
            reason: "csv codec: failed to read byte source",
        })?;
        let decoded = self
            .decode(&raw)
            .map_err(|_| TranslateEditError::Internal {
                reason: "csv codec: decode failed during translate_edit",
            })?;

        let start_offset = text_position_to_offset(&decoded.content, start).ok_or(
            TranslateEditError::ConstraintViolation {
                reason: "csv codec: start position out of range",
            },
        )?;
        let end_offset = text_position_to_offset(&decoded.content, end).ok_or(
            TranslateEditError::ConstraintViolation {
                reason: "csv codec: end position out of range",
            },
        )?;
        if end_offset < start_offset {
            return Err(TranslateEditError::ConstraintViolation {
                reason: "csv codec: end offset precedes start offset",
            });
        }

        let mut edited = decoded.content.clone();
        edited.replace_range(start_offset..end_offset, replacement);

        let new_bytes = self.encode_fragment(&edited, &decoded.metadata);

        let old_bytes = raw;
        Ok(Some(ByteEdit {
            offset: 0,
            old_bytes,
            new_bytes,
        }))
    }
}

impl CsvCodec {
    /// Internal helper used by [`ContentCodec::translate_edit`] to
    /// re-encode a decoded CSV body back into bytes.
    ///
    /// Retained as an inherent method after `#740` Plan 06 Phase 5
    /// sub-commit 5d deleted the public `ContentCodec::encode` seam.
    fn encode_fragment(&self, content: &str, metadata: &CodecMetadata) -> Vec<u8> {
        let delimiter = metadata
            .get("delimiter")
            .and_then(|s| s.chars().next())
            .unwrap_or(self.delimiter as char);

        let line_ending = match metadata.get("line_ending") {
            Some("crlf") => "\r\n",
            _ => "\n",
        };

        encode_csv(content, delimiter, line_ending)
    }
}

fn text_position_to_offset(text: &str, pos: &reovim_domain_text::Position) -> Option<usize> {
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

fn read_all_bytes(bytes: &dyn reovim_driver_vfs::ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();
    (data.len() == len).then_some(data)
}

/// Parse CSV text into rows of fields.
///
/// Returns the parsed rows and whether the first row looks like a header
/// (contains non-numeric values while other rows contain numbers).
fn parse_csv(text: &str, delimiter: u8) -> (Vec<Vec<String>>, bool) {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let rows: Vec<Vec<String>> = reader
        .records()
        .filter_map(Result::ok)
        .map(|record| record.iter().map(String::from).collect())
        .collect();

    let has_header = detect_header(&rows);
    (rows, has_header)
}

/// Detect if the first row is a header by checking if it contains
/// non-numeric values while subsequent rows have more numeric values.
#[cfg_attr(coverage_nightly, coverage(off))]
fn detect_header(rows: &[Vec<String>]) -> bool {
    if rows.len() < 2 {
        return false;
    }

    let first_row = &rows[0];
    let first_numeric = first_row
        .iter()
        .filter(|f| f.parse::<f64>().is_ok())
        .count();

    // If first row has fewer numeric fields than data rows, it's likely a header
    if let Some(second_row) = rows.get(1) {
        let second_numeric = second_row
            .iter()
            .filter(|f| f.parse::<f64>().is_ok())
            .count();
        return first_numeric < second_numeric;
    }

    false
}

/// Format parsed rows into a column-aligned table.
#[cfg_attr(coverage_nightly, coverage(off))]
fn format_table(rows: &[Vec<String>], has_header: bool) -> (String, Vec<Annotation>) {
    if rows.is_empty() {
        return (String::new(), Vec::new());
    }

    // Calculate column widths
    let col_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0_usize; col_count];

    for row in rows {
        for (i, field) in row.iter().enumerate() {
            if i < col_count {
                widths[i] = widths[i].max(field.len());
            }
        }
    }

    // Minimum column width of 3
    for w in &mut widths {
        *w = (*w).max(3);
    }

    let mut output = String::with_capacity(rows.len() * col_count * 10);
    let mut annotations = Vec::new();

    let header_kind = AnnotationKind::new(CSV_HEADER_KIND);
    let column_kind = AnnotationKind::new(CSV_COLUMN_KIND);

    for (line_idx, row) in rows.iter().enumerate() {
        // Format each field with padding
        for (col_idx, field) in row.iter().enumerate() {
            if col_idx > 0 {
                output.push_str("  "); // Column separator
            }
            let width = widths.get(col_idx).copied().unwrap_or(3);
            let _ = write!(output, "{field:<width$}");
        }
        output.push('\n');

        // Annotate header row
        if line_idx == 0 && has_header {
            annotations.push(Annotation {
                kind: header_kind.clone(),
                target: AnnotationTarget::Line(line_idx),
                priority: 0,
                payload: AnnotationPayload::None,
            });
        }

        // Annotate each column
        annotations.push(Annotation {
            kind: column_kind.clone(),
            target: AnnotationTarget::Line(line_idx),
            priority: 0,
            payload: AnnotationPayload::Number(row.len()),
        });
    }

    (output, annotations)
}

/// Encode column-aligned text back to delimited format.
#[cfg_attr(coverage_nightly, coverage(off))]
fn encode_csv(content: &str, delimiter: char, line_ending: &str) -> Vec<u8> {
    let mut result = Vec::with_capacity(content.len());

    for line in content.lines() {
        // Split on multiple spaces (column separator in aligned view)
        let fields: Vec<&str> = split_aligned_fields(line);

        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                result.push(delimiter as u8);
            }
            let trimmed = field.trim();

            // Quote if field contains delimiter, quote, or newline (RFC 4180 §2.6)
            if trimmed.contains(delimiter)
                || trimmed.contains('"')
                || trimmed.contains('\n')
                || trimmed.contains('\r')
            {
                result.push(b'"');
                for ch in trimmed.bytes() {
                    if ch == b'"' {
                        result.push(b'"');
                    }
                    result.push(ch);
                }
                result.push(b'"');
            } else {
                result.extend_from_slice(trimmed.as_bytes());
            }
        }

        result.extend_from_slice(line_ending.as_bytes());
    }

    result
}

/// Split an aligned table line back into fields.
///
/// Fields in the aligned view are separated by 2+ spaces. A single
/// space within a field is preserved.
#[cfg_attr(coverage_nightly, coverage(off))]
fn split_aligned_fields(line: &str) -> Vec<&str> {
    if line.is_empty() {
        return Vec::new();
    }

    let mut fields = Vec::new();
    let mut start = 0;
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Look for 2+ consecutive spaces as field separator
        if i + 1 < bytes.len() && bytes[i] == b' ' && bytes[i + 1] == b' ' {
            // Found field separator — capture the field
            fields.push(line[start..i].trim_end());

            // Skip all separator spaces
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            start = i;
        } else {
            i += 1;
        }
    }

    // Capture the last field
    if start < bytes.len() {
        fields.push(line[start..].trim_end());
    } else if !fields.is_empty() {
        // Trailing separator — add empty field
        fields.push("");
    }

    fields
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
