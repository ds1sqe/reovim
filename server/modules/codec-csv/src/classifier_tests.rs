//! Tests for CSV classifier.

use reovim_driver_codec::{ContentClassifier, ContentType};

use super::*;

#[test]
fn csv_by_content() {
    let c = CsvClassifier::new();
    let data = b"name,age,city\nAlice,30,NYC\nBob,25,LA\n";
    assert_eq!(c.classify(data, "data.txt"), Some(ContentType::new(CSV)));
}

#[test]
fn tsv_by_content() {
    let c = CsvClassifier::new();
    let data = b"name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA\n";
    assert_eq!(c.classify(data, "data.txt"), Some(ContentType::new(TSV)));
}

#[test]
fn psv_by_content() {
    let c = CsvClassifier::new();
    let data = b"name|age|city\nAlice|30|NYC\nBob|25|LA\n";
    assert_eq!(c.classify(data, "data.txt"), Some(ContentType::new(PSV)));
}

#[test]
fn semicolon_csv_by_content() {
    let c = CsvClassifier::new();
    let data = b"name;age;city\nAlice;30;NYC\nBob;25;LA\n";
    assert_eq!(c.classify(data, "data.txt"), Some(ContentType::new(SCSV)));
}

#[test]
fn csv_extension_fast_path() {
    let c = CsvClassifier::new();
    assert_eq!(
        c.classify(b"not csv", "data.csv"),
        Some(ContentType::new(CSV))
    );
}

#[test]
fn tsv_extension_fast_path() {
    let c = CsvClassifier::new();
    assert_eq!(
        c.classify(b"not tsv", "data.tsv"),
        Some(ContentType::new(TSV))
    );
}

#[test]
fn tab_extension_fast_path() {
    let c = CsvClassifier::new();
    assert_eq!(
        c.classify(b"not tsv", "data.tab"),
        Some(ContentType::new(TSV))
    );
}

#[test]
fn psv_extension_fast_path() {
    let c = CsvClassifier::new();
    assert_eq!(
        c.classify(b"not psv", "data.psv"),
        Some(ContentType::new(PSV))
    );
}

#[test]
fn extension_case_insensitive() {
    let c = CsvClassifier::new();
    assert_eq!(
        c.classify(b"data", "file.CSV"),
        Some(ContentType::new(CSV))
    );
}

#[test]
fn plain_text_not_csv() {
    let c = CsvClassifier::new();
    assert!(c.classify(b"Hello, world!\nThis is text.\n", "test.txt").is_none());
}

#[test]
fn single_line_not_csv() {
    let c = CsvClassifier::new();
    assert!(c.classify(b"a,b,c", "test.txt").is_none());
}

#[test]
fn single_column_not_csv() {
    let c = CsvClassifier::new();
    assert!(c.classify(b"hello\nworld\nfoo\n", "test.txt").is_none());
}

#[test]
fn inconsistent_columns_not_csv() {
    let c = CsvClassifier::new();
    let data = b"a,b,c\nx,y\n";
    assert!(c.classify(data, "test.txt").is_none());
}

#[test]
fn empty_input() {
    let c = CsvClassifier::new();
    assert!(c.classify(b"", "test.txt").is_none());
}

#[test]
fn binary_not_csv() {
    let c = CsvClassifier::new();
    assert!(c.classify(b"\x00\x01\x02", "test.bin").is_none());
}

#[test]
fn priority_is_15() {
    let c = CsvClassifier::new();
    assert_eq!(c.priority(), 15);
}

#[test]
fn name_is_csv() {
    let c = CsvClassifier::new();
    assert_eq!(c.name(), "csv");
}

#[test]
fn default_impl() {
    let c = CsvClassifier;
    assert_eq!(c.name(), "csv");
}

#[test]
fn extension_content_type_tests() {
    assert_eq!(extension_content_type("data.csv"), Some(CSV));
    assert_eq!(extension_content_type("data.tsv"), Some(TSV));
    assert_eq!(extension_content_type("data.tab"), Some(TSV));
    assert_eq!(extension_content_type("data.psv"), Some(PSV));
    assert_eq!(extension_content_type("data.txt"), None);
    assert_eq!(extension_content_type("Makefile"), None);
}

#[test]
fn is_consistent_delimiter_comma() {
    let lines = vec!["a,b,c", "1,2,3", "x,y,z"];
    assert!(is_consistent_delimiter(&lines, b','));
}

#[test]
fn is_consistent_delimiter_inconsistent() {
    let lines = vec!["a,b,c", "1,2"];
    assert!(!is_consistent_delimiter(&lines, b','));
}

#[test]
fn is_consistent_delimiter_single_column() {
    let lines = vec!["hello", "world"];
    assert!(!is_consistent_delimiter(&lines, b','));
}

#[test]
fn is_consistent_delimiter_empty_lines_skipped() {
    let lines = vec!["a,b", "", "c,d"];
    assert!(is_consistent_delimiter(&lines, b','));
}

#[test]
fn is_consistent_delimiter_too_few_lines() {
    let lines = vec!["a,b"];
    assert!(!is_consistent_delimiter(&lines, b','));
}

#[test]
fn with_path() {
    let c = CsvClassifier::new();
    assert_eq!(
        c.classify(b"data", "/home/user/data.csv"),
        Some(ContentType::new(CSV))
    );
}
