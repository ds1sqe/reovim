//! Tests for CSV codec factory.

use reovim_driver_codec::{ContentCodecFactory, ContentType};

use super::CsvCodecFactory;

#[test]
fn creates_csv_codec() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new(crate::classifier::CSV);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_tsv_codec() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new(crate::classifier::TSV);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_psv_codec() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new(crate::classifier::PSV);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_scsv_codec() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new(crate::classifier::SCSV);
    let codec = factory.create(&ct);
    assert!(codec.is_some());
    // Verify it decodes with semicolon delimiter
    let result = codec.unwrap().decode(b"a;b\n1;2\n").unwrap();
    assert!(result.content.contains('a'));
    assert!(result.content.contains('b'));
}

#[test]
fn rejects_binary_raw() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_utf8() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_unknown() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new("application/json");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_content_types() {
    let factory = CsvCodecFactory::new();
    let types = factory.supported_content_types();
    assert!(types.contains(&crate::classifier::CSV));
    assert!(types.contains(&crate::classifier::TSV));
    assert!(types.contains(&crate::classifier::PSV));
    assert!(types.contains(&crate::classifier::SCSV));
    assert_eq!(types.len(), 4);
}

#[test]
fn name() {
    let factory = CsvCodecFactory::new();
    assert_eq!(factory.name(), "csv");
}

#[test]
fn csv_codec_is_bidirectional() {
    let factory = CsvCodecFactory::new();
    let ct = ContentType::new(crate::classifier::CSV);
    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(b"a,b\n1,2\n").unwrap();
    // Bidirectional: encode should return Some
    assert!(codec.encode(&result.content, &result.metadata).is_some());
}

#[test]
fn default_impl() {
    let factory = CsvCodecFactory;
    assert_eq!(factory.name(), "csv");
}
