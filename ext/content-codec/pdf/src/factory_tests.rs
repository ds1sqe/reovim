//! Tests for PDF codec factory.

use reovim_content_codec::{ContentCodecFactory, ContentType};

use super::PdfCodecFactory;

#[test]
fn creates_pdf_codec_for_document_pdf() {
    let factory = PdfCodecFactory::new();
    let ct = ContentType::new(crate::classifier::PDF);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_binary_raw() {
    let factory = PdfCodecFactory::new();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_utf8() {
    let factory = PdfCodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_unknown_type() {
    let factory = PdfCodecFactory::new();
    let ct = ContentType::new("application/json");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_content_types() {
    let factory = PdfCodecFactory::new();
    assert_eq!(factory.supported_content_types(), vec![crate::classifier::PDF]);
}

#[test]
fn name() {
    let factory = PdfCodecFactory::new();
    assert_eq!(factory.name(), "pdf");
}

#[test]
fn default_impl() {
    let factory = PdfCodecFactory;
    assert_eq!(factory.name(), "pdf");
}
