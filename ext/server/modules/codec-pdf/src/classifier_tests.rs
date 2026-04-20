//! Tests for PDF classifier.

use reovim_driver_codec::{ContentClassifier, ContentType};

use super::*;

#[test]
fn magic_bytes_detected() {
    let c = PdfClassifier::new();
    let data = b"%PDF-1.7\n";
    assert_eq!(c.classify(data, "unknown"), Some(ContentType::new(PDF)));
}

#[test]
fn magic_bytes_version_1_4() {
    let c = PdfClassifier::new();
    let data = b"%PDF-1.4 some content";
    assert_eq!(c.classify(data, "unknown"), Some(ContentType::new(PDF)));
}

#[test]
fn magic_bytes_version_2_0() {
    let c = PdfClassifier::new();
    let data = b"%PDF-2.0\x00\x00";
    assert_eq!(c.classify(data, "doc.bin"), Some(ContentType::new(PDF)));
}

#[test]
fn pdf_extension_fast_path() {
    let c = PdfClassifier::new();
    // Even with non-PDF content, .pdf extension triggers classification
    assert_eq!(c.classify(b"not a pdf", "document.pdf"), Some(ContentType::new(PDF)));
}

#[test]
fn pdf_extension_case_insensitive() {
    let c = PdfClassifier::new();
    assert_eq!(c.classify(b"data", "doc.PDF"), Some(ContentType::new(PDF)));
}

#[test]
fn pdf_extension_with_path() {
    let c = PdfClassifier::new();
    assert_eq!(c.classify(b"data", "/home/user/docs/report.pdf"), Some(ContentType::new(PDF)));
}

#[test]
fn non_pdf_content_no_extension() {
    let c = PdfClassifier::new();
    assert!(c.classify(b"Hello, world!", "test.txt").is_none());
}

#[test]
fn binary_content_not_pdf() {
    let c = PdfClassifier::new();
    assert!(
        c.classify(b"\x7fELF\x02\x01\x01\x00", "binary.bin")
            .is_none()
    );
}

#[test]
fn empty_input() {
    let c = PdfClassifier::new();
    assert!(c.classify(b"", "test.txt").is_none());
}

#[test]
fn too_short_for_magic() {
    let c = PdfClassifier::new();
    assert!(c.classify(b"%PDF", "test.txt").is_none());
}

#[test]
fn partial_magic_not_detected() {
    let c = PdfClassifier::new();
    assert!(c.classify(b"%PDE-1.7", "test.txt").is_none());
}

#[test]
fn priority_is_35() {
    let c = PdfClassifier::new();
    assert_eq!(c.priority(), 35);
}

#[test]
fn name_is_pdf() {
    let c = PdfClassifier::new();
    assert_eq!(c.name(), "pdf");
}

#[test]
fn no_extension() {
    let c = PdfClassifier::new();
    assert!(c.classify(b"hello", "Makefile").is_none());
}

#[test]
fn has_pdf_extension_tests() {
    assert!(has_pdf_extension("test.pdf"));
    assert!(has_pdf_extension("test.PDF"));
    assert!(!has_pdf_extension("test.txt"));
    assert!(!has_pdf_extension("test.rs"));
    assert!(!has_pdf_extension("Makefile"));
}

#[test]
fn default_impl() {
    let c = PdfClassifier;
    assert_eq!(c.name(), "pdf");
}
