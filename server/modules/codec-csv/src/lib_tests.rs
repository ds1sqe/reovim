//! Module-level tests for codec-csv.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecCsvModule::new();
    assert_eq!(m.id().as_str(), "codec-csv");
}

#[test]
fn module_name() {
    let m = CodecCsvModule::new();
    assert_eq!(m.name(), "Codec CSV");
}

#[test]
fn module_version() {
    let m = CodecCsvModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecCsvModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate() {
    let classifier = CsvClassifier::new();
    let factory = CsvCodecFactory::new();

    let data = b"name,age\nAlice,30\nBob,25\n";
    let ct = classifier.classify(data, "data.txt").unwrap();
    assert_eq!(ct.as_str(), crate::classifier::CSV);

    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(data).unwrap();
    assert!(result.content.contains("Alice"));
    assert!(!result.lossy);
    assert!(!result.readonly);

    // Bidirectional: can encode back
    let encoded = codec.encode(&result.content, &result.metadata);
    assert!(encoded.is_some());
}

#[test]
fn text_not_classified_as_csv() {
    let classifier = CsvClassifier::new();
    assert!(classifier
        .classify(b"Hello world\nSome text\n", "test.txt")
        .is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecCsvModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecCsvModule;
    assert_eq!(m.id().as_str(), "codec-csv");
}
