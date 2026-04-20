//! Tests for CSV codec.

use {
    reovim_domain_text::Position,
    reovim_content_codec::{CodecMetadata, ContentCodec, ContentType, DecodedEdit, DomainEdit},
    reovim_content_codec_text::TextEdit,
    reovim_subsys_vfs::HeapByteSource,
};

use {
    super::*,
    crate::classifier::{CSV, PSV, TSV},
};

#[test]
fn decode_simple_csv() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"name,age\nAlice,30\nBob,25\n").unwrap();
    assert!(result.content.contains("name"));
    assert!(result.content.contains("Alice"));
    assert!(result.content.contains("Bob"));
    assert!(!result.lossy);
    assert!(!result.readonly);
}

#[test]
fn decode_tsv() {
    let codec = CsvCodec::new(b'\t', TSV);
    let result = codec.decode(b"name\tage\nAlice\t30\n").unwrap();
    assert!(result.content.contains("name"));
    assert!(result.content.contains("Alice"));
}

#[test]
fn decode_psv() {
    let codec = CsvCodec::new(b'|', PSV);
    let result = codec.decode(b"name|age\nAlice|30\n").unwrap();
    assert!(result.content.contains("name"));
    assert!(result.content.contains("Alice"));
}

#[test]
fn decode_metadata_delimiter() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"a,b\n1,2\n").unwrap();
    assert_eq!(result.metadata.get("delimiter"), Some(","));
}

#[test]
fn decode_metadata_line_ending_lf() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"a,b\n1,2\n").unwrap();
    assert_eq!(result.metadata.get("line_ending"), Some("lf"));
}

#[test]
fn decode_metadata_line_ending_crlf() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"a,b\r\n1,2\r\n").unwrap();
    assert_eq!(result.metadata.get("line_ending"), Some("crlf"));
}

#[test]
fn decode_header_detection() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"name,age\nAlice,30\n").unwrap();
    assert_eq!(result.metadata.get("has_header"), Some("true"));
}

#[test]
fn decode_no_header_all_numeric() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"1,2\n3,4\n").unwrap();
    assert_eq!(result.metadata.get("has_header"), Some("false"));
}

#[test]
fn decode_column_alignment() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"name,age\nAlice,30\n").unwrap();
    let lines: Vec<&str> = result.content.lines().collect();
    assert_eq!(lines.len(), 2);
    // Both lines should have the same length (padded by format!)
    // name  should be padded to match "Alice"
    assert_eq!(lines[0].len(), lines[1].len());
}

#[test]
fn decode_annotations_header() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"name,age\nAlice,30\n").unwrap();

    let header_annotations: usize = result
        .annotations
        .iter()
        .filter(|a| a.kind.name() == CSV_HEADER_KIND)
        .count();
    assert_eq!(header_annotations, 1);
}

#[test]
fn decode_annotations_columns() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"a,b,c\n1,2,3\n").unwrap();

    let col_annotations: usize = result
        .annotations
        .iter()
        .filter(|a| a.kind.name() == CSV_COLUMN_KIND)
        .count();
    assert_eq!(col_annotations, 2); // One per row
}

#[test]
fn decode_namespace() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"a,b\n1,2\n").unwrap();
    for a in &result.annotations {
        assert_eq!(a.kind.namespace(), Some("content"));
    }
}

#[test]
fn encode_simple_csv() {
    let codec = CsvCodec::new(b',', CSV);
    let mut metadata = CodecMetadata::new(ContentType::new(CSV));
    metadata.set("delimiter", ",");
    metadata.set("line_ending", "lf");

    let result = codec.encode_fragment("Alice  30\nBob    25\n", &metadata);
    let text = String::from_utf8(result).unwrap();
    assert!(text.contains("Alice,30"));
    assert!(text.contains("Bob,25"));
}

#[test]
fn encode_crlf() {
    let codec = CsvCodec::new(b',', CSV);
    let mut metadata = CodecMetadata::new(ContentType::new(CSV));
    metadata.set("delimiter", ",");
    metadata.set("line_ending", "crlf");

    let result = codec.encode_fragment("a  b\n", &metadata);
    let text = String::from_utf8(result).unwrap();
    assert!(text.contains("\r\n"));
}

#[test]
fn encode_tab_delimiter() {
    let codec = CsvCodec::new(b'\t', TSV);
    let mut metadata = CodecMetadata::new(ContentType::new(TSV));
    metadata.set("delimiter", "\t");
    metadata.set("line_ending", "lf");

    let result = codec.encode_fragment("Alice  30\n", &metadata);
    let text = String::from_utf8(result).unwrap();
    assert!(text.contains("Alice\t30"));
}

#[test]
fn decode_invalid_utf8() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(&[0xFF, 0xFE, 0x00]);
    assert!(result.is_err());
}

#[test]
fn decode_empty() {
    let codec = CsvCodec::new(b',', CSV);
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
}

#[test]
fn delimiter_accessor() {
    let codec = CsvCodec::new(b',', CSV);
    assert_eq!(codec.delimiter(), b',');
}

#[test]
fn format_table_empty() {
    let (content, annotations) = format_table(&[], false);
    assert_eq!(content, "");
    assert!(annotations.is_empty());
}

#[test]
fn detect_header_too_few_rows() {
    let rows: Vec<Vec<String>> = vec![vec!["a".into(), "b".into()]];
    assert!(!detect_header(&rows));
}

#[test]
fn detect_header_all_text() {
    let rows = vec![
        vec!["name".into(), "city".into()],
        vec!["Alice".into(), "NYC".into()],
    ];
    assert!(!detect_header(&rows));
}

#[test]
fn detect_header_text_vs_numeric() {
    let rows = vec![
        vec!["name".into(), "age".into()],
        vec!["Alice".into(), "30".into()],
    ];
    assert!(detect_header(&rows));
}

#[test]
fn split_aligned_fields_simple() {
    let fields = split_aligned_fields("Alice  30  NYC");
    assert_eq!(fields, vec!["Alice", "30", "NYC"]);
}

#[test]
fn split_aligned_fields_empty() {
    let fields = split_aligned_fields("");
    assert!(fields.is_empty());
}

#[test]
fn split_aligned_fields_single() {
    let fields = split_aligned_fields("hello");
    assert_eq!(fields, vec!["hello"]);
}

#[test]
fn split_aligned_fields_preserves_single_space() {
    let fields = split_aligned_fields("New York  30");
    assert_eq!(fields, vec!["New York", "30"]);
}

#[test]
fn encode_quoting() {
    let result = encode_csv("hello  world,test\n", ',', "\n");
    let text = String::from_utf8(result).unwrap();
    // "world,test" should be quoted because it contains a comma
    assert!(text.contains("\"world,test\""));
}

#[test]
fn round_trip_simple() {
    let codec = CsvCodec::new(b',', CSV);
    let original = b"name,age\nAlice,30\nBob,25\n";
    let decoded = codec.decode(original).unwrap();
    let encoded = codec.encode_fragment(&decoded.content, &decoded.metadata);
    let re_decoded = codec.decode(&encoded).unwrap();

    // Content should be identical after round-trip
    assert_eq!(decoded.content, re_decoded.content);
}

#[test]
fn translate_edit_text_replaces_value() {
    let codec = CsvCodec::new(b',', CSV);
    let original = b"name,age\nAlice,30\nBob,25\n";
    let decoded = codec.decode(original).unwrap();

    let start = decoded
        .content
        .find("30")
        .expect("value expected in decoded view");
    let end = start + 2;
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: position_at_byte_index(&decoded.content, start),
        end: position_at_byte_index(&decoded.content, end),
        replacement: "31".to_string(),
    }));

    let translated = codec
        .translate_edit(&HeapByteSource::new(original), &edit)
        .expect("CSV text edits should translate")
        .expect("produced a byte edit");

    let mut expected_content = decoded.content.clone();
    expected_content.replace_range(start..end, "31");
    let expected_new_bytes = codec.encode_fragment(&expected_content, &decoded.metadata);

    assert_eq!(translated.offset, 0);
    assert_eq!(translated.old_bytes, original.to_vec());
    assert_eq!(translated.new_bytes, expected_new_bytes);
}

#[test]
fn translate_edit_bytes_variant_is_not_supported() {
    use reovim_content_codec::TranslateEditError;
    let codec = CsvCodec::new(b',', CSV);
    let bytes = HeapByteSource::new(b"name,age\n");
    let edit = DecodedEdit::Bytes {
        offset: 1,
        old_len: 1,
        new_bytes: b"X".to_vec(),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_tree_variant_is_not_supported() {
    use reovim_content_codec::{TranslateEditError, TreeOp, TreePath};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestOp;
    reovim_content_codec::impl_tree_op!(TestOp);

    let codec = CsvCodec::new(b',', CSV);
    let bytes = HeapByteSource::new(b"name,age\n");
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::new(TestOp),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_end_before_start_rejected() {
    use reovim_content_codec::TranslateEditError;
    let codec = CsvCodec::new(b',', CSV);
    let bytes = HeapByteSource::new(b"name,age\nBob,30\n");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 4),
        end: Position::new(0, 0),
        replacement: "x".to_string(),
    }));

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_start_out_of_range() {
    use reovim_content_codec::TranslateEditError;
    let codec = CsvCodec::new(b',', CSV);
    let bytes = HeapByteSource::new(b"name,age\nBob,30\n");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(99, 0),
        end: Position::new(99, 1),
        replacement: "x".to_string(),
    }));

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_decode_failure_returns_internal_error() {
    use reovim_content_codec::TranslateEditError;
    let codec = CsvCodec::new(b',', CSV);
    // Invalid UTF-8 causes csv decode to fail.
    let bytes = HeapByteSource::new([0xFF, 0xFE, 0x00]);
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
        replacement: "x".to_string(),
    }));

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::Internal { .. })
    ));
}

#[test]
fn translate_edit_end_out_of_range() {
    use reovim_content_codec::TranslateEditError;
    let codec = CsvCodec::new(b',', CSV);
    let bytes = HeapByteSource::new(b"name,age\nBob,30\n");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 0),
        end: Position::new(99, 0),
        replacement: "x".to_string(),
    }));

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_column_at_line_end() {
    // Exercises codec.rs line 191:
    //   `(chars_seen == pos.column).then_some(line_end)`
    //
    // This branch executes when `pos.column` equals the character count
    // of the line — i.e. the edit position sits exactly at the end of the
    // line (past the last char, but still on that line).  Existing tests
    // all target positions strictly inside the line, so this branch was
    // previously unreachable.
    let codec = CsvCodec::new(b',', CSV);

    // A single-value CSV.  `format_table` pads to max(len, 3) and appends
    // '\n', so "hello" decodes to "hello\n".  The line body has 5 chars.
    let csv_bytes = b"hello\n";
    let decoded = codec.decode(csv_bytes).unwrap();
    let line_len = decoded
        .content
        .lines()
        .next()
        .expect("decoded content has at least one line")
        .chars()
        .count();

    // Sanity: `line_len` must be > 0 for the loop to exhaust before
    // matching, so `then_some` is actually evaluated.
    assert!(line_len > 0, "expected non-empty line in decoded form");

    let bytes = HeapByteSource::new(csv_bytes.as_slice());
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, line_len),
        end: Position::new(0, line_len),
        replacement: String::new(),
    }));

    // The result must be Ok — a no-op insertion at end-of-line is valid.
    let result = codec
        .translate_edit(&bytes, &edit)
        .expect("end-of-line position is valid");
    assert!(result.is_some(), "expected a byte edit to be produced");
}

fn position_at_byte_index(text: &str, target: usize) -> Position {
    let mut line = 0usize;
    let mut column = 0usize;

    let bytes = text.as_bytes();
    assert!(target <= bytes.len());

    for &byte in &bytes[..target] {
        if byte == b'\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Position::new(line, column)
}
