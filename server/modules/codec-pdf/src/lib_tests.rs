//! Module-level tests for codec-pdf.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory, ContentType},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecPdfModule::new();
    assert_eq!(m.id().as_str(), "codec-pdf");
}

#[test]
fn module_name() {
    let m = CodecPdfModule::new();
    assert_eq!(m.name(), "Codec PDF");
}

#[test]
fn module_version() {
    let m = CodecPdfModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecPdfModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate() {
    let classifier = PdfClassifier::new();
    let factory = PdfCodecFactory::new();

    // Classify PDF by extension
    let ct = classifier.classify(b"%PDF-1.7", "report.pdf").unwrap();
    assert_eq!(ct.as_str(), crate::classifier::PDF);

    // Factory creates codec for that content type
    let _codec = factory.create(&ct).unwrap();
}

#[test]
fn classifier_passes_text_factory_rejects_text() {
    let classifier = PdfClassifier::new();
    let factory = PdfCodecFactory::new();

    // Plain text not classified as PDF
    assert!(classifier.classify(b"hello world", "test.txt").is_none());

    // UTF-8 content type rejected by PDF factory
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecPdfModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecPdfModule;
    assert_eq!(m.id().as_str(), "codec-pdf");
}
