//! Tests for tar.gz codec factory.

use reovim_content_codec::{ContentCodecFactory, ContentType};

use super::*;

#[test]
fn creates_tar_gz_codec() {
    let factory = TarGzCodecFactory::new();
    let ct = ContentType::new(TAR_GZ);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_binary_raw() {
    let factory = TarGzCodecFactory::new();
    assert!(factory.create(&ContentType::new("binary/raw")).is_none());
}

#[test]
fn rejects_binary_zip() {
    let factory = TarGzCodecFactory::new();
    assert!(factory.create(&ContentType::new("binary/zip")).is_none());
}

#[test]
fn rejects_text_utf8() {
    let factory = TarGzCodecFactory::new();
    assert!(factory.create(&ContentType::new("text/utf-8")).is_none());
}

#[test]
fn rejects_unknown() {
    let factory = TarGzCodecFactory::new();
    assert!(factory.create(&ContentType::new("unknown/type")).is_none());
}

#[test]
fn supported_content_types() {
    let factory = TarGzCodecFactory::new();
    let types = factory.supported_content_types();
    assert_eq!(types, vec![TAR_GZ]);
}

#[test]
fn name_is_tar_gz() {
    let factory = TarGzCodecFactory::new();
    assert_eq!(factory.name(), "tar-gz");
}

#[test]
fn default_impl() {
    let factory = TarGzCodecFactory;
    assert_eq!(factory.name(), "tar-gz");
}
