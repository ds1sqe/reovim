//! Tests for CSV codec.

use reovim_driver_codec::{CodecMetadata, ContentCodec, ContentType};

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

    let result = codec
        .encode("Alice  30\nBob    25\n", &metadata)
        .unwrap()
        .unwrap();
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

    let result = codec.encode("a  b\n", &metadata).unwrap().unwrap();
    let text = String::from_utf8(result).unwrap();
    assert!(text.contains("\r\n"));
}

#[test]
fn encode_tab_delimiter() {
    let codec = CsvCodec::new(b'\t', TSV);
    let mut metadata = CodecMetadata::new(ContentType::new(TSV));
    metadata.set("delimiter", "\t");
    metadata.set("line_ending", "lf");

    let result = codec.encode("Alice  30\n", &metadata).unwrap().unwrap();
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
    let encoded = codec
        .encode(&decoded.content, &decoded.metadata)
        .unwrap()
        .unwrap();
    let re_decoded = codec.decode(&encoded).unwrap();

    // Content should be identical after round-trip
    assert_eq!(decoded.content, re_decoded.content);
}
