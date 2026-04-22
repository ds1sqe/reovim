//! Module-level tests for codec-csv.

use {
    reovim_content_codec::{ContentClassifier, ContentCodecFactory, ContentType},
    reovim_kernel::api::v1::{Module, ModuleContext},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

use super::*;

fn test_ctx() -> ModuleContext {
    ModuleContext::default()
}

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
    assert!(
        m.provides()
            .contains(&reovim_subsys_content_codec::capabilities::CODEC_PROVIDER)
    );
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
}

#[test]
fn text_not_classified_as_csv() {
    let classifier = CsvClassifier::new();
    assert!(
        classifier
            .classify(b"Hello world\nSome text\n", "test.txt")
            .is_none()
    );
}

#[test]
fn default_has_no_registry_handle() {
    let m = CodecCsvModule::default();
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());
}

#[test]
fn init_acquires_registry_and_registers_all_codecs() {
    let ctx = test_ctx();
    let mut m = CodecCsvModule::new();
    assert!(matches!(m.init(&ctx), reovim_kernel::api::v1::ProbeResult::Success));
    assert!(m.registry.is_some());
    assert_eq!(m.content_types.len(), 4);

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    for ct_str in &["text/csv", "text/tsv", "text/psv", "text/scsv"] {
        assert!(
            registry.get(&ContentType::new(*ct_str)).is_some(),
            "expected {ct_str} to be registered"
        );
    }
}

#[test]
fn exit_releases_registry_and_unregisters_all_codecs() {
    let ctx = test_ctx();
    let mut m = CodecCsvModule::new();
    let _ = m.init(&ctx);

    m.exit().expect("exit should succeed");
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    for ct_str in &["text/csv", "text/tsv", "text/psv", "text/scsv"] {
        assert!(
            registry.get(&ContentType::new(*ct_str)).is_none(),
            "expected {ct_str} to be unregistered"
        );
    }
}

#[test]
fn exit_without_init_is_noop() {
    let mut m = CodecCsvModule::new();
    m.exit().expect("exit should succeed even without init");
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut m = CodecCsvModule::new();
    let _ = m.init(&ctx);
    m.exit().expect("first exit should succeed");
    m.exit().expect("second exit should succeed (idempotent)");
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());
}

#[test]
fn module_exit() {
    let mut m = CodecCsvModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecCsvModule::default();
    assert_eq!(m.id().as_str(), "codec-csv");
}
