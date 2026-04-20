//! Tests for rlib codec factory.

use reovim_content_codec::{ContentCodecFactory, ContentType};

use super::*;

#[test]
fn creates_rlib_codec() {
    let factory = RlibCodecFactory::new();
    let ct = ContentType::new(RLIB);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_binary_raw() {
    let factory = RlibCodecFactory::new();
    assert!(factory.create(&ContentType::new("binary/raw")).is_none());
}

#[test]
fn rejects_binary_elf() {
    let factory = RlibCodecFactory::new();
    assert!(factory.create(&ContentType::new("binary/elf")).is_none());
}

#[test]
fn rejects_text_utf8() {
    let factory = RlibCodecFactory::new();
    assert!(factory.create(&ContentType::new("text/utf-8")).is_none());
}

#[test]
fn rejects_unknown() {
    let factory = RlibCodecFactory::new();
    assert!(factory.create(&ContentType::new("unknown/type")).is_none());
}

#[test]
fn supported_content_types() {
    let factory = RlibCodecFactory::new();
    let types = factory.supported_content_types();
    assert_eq!(types, vec![RLIB]);
}

#[test]
fn name_is_rlib() {
    let factory = RlibCodecFactory::new();
    assert_eq!(factory.name(), "rlib");
}

#[test]
fn default_impl() {
    let factory = RlibCodecFactory;
    assert_eq!(factory.name(), "rlib");
}

// The historic `created_codec_is_one_way` test asserted that the factory-
// produced codec's `ContentCodec::encode` returned `None`. That trait
// method was deleted in #740 Plan 06 Phase 5 sub-commit 5d — read-only
// codecs are now expressed as "no `translate_edit` override" and the
// compiler enforces the contract structurally, so the runtime assertion
// is no longer representable.
