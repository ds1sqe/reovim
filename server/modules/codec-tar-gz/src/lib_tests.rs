//! Module-level tests for codec-tar-gz.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory},
    reovim_kernel::api::v1::Module,
};

use super::*;

const GZIP_HEADER: &[u8] = &[0x1f, 0x8b, 0x08, 0x00];

#[test]
fn module_id() {
    let m = CodecTarGzModule::new();
    assert_eq!(m.id().as_str(), "codec-tar-gz");
}

#[test]
fn module_name() {
    let m = CodecTarGzModule::new();
    assert_eq!(m.name(), "Codec Tar-Gz");
}

#[test]
fn module_version() {
    let m = CodecTarGzModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecTarGzModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate() {
    let classifier = TarGzClassifier::new();
    let factory = TarGzCodecFactory::new();

    let ct = classifier.classify(GZIP_HEADER, "archive.tar.gz").unwrap();
    assert_eq!(ct.as_str(), crate::classifier::TAR_GZ);

    // Factory creates codec for tar-gz type
    let _codec = factory.create(&ct).unwrap();
}

#[test]
fn text_not_classified() {
    let c = TarGzClassifier::new();
    assert!(c.classify(b"hello world", "test.txt").is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecTarGzModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecTarGzModule;
    assert_eq!(m.id().as_str(), "codec-tar-gz");
}
