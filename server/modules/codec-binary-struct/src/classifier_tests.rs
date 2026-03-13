//! Tests for ELF and ZIP classifiers.

use reovim_driver_codec::{ContentClassifier, ContentType};

use super::*;

// === ELF Classifier Tests ===

#[test]
fn elf_magic_detected() {
    let c = ElfClassifier::new();
    let data = b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    assert_eq!(c.classify(data, "unknown"), Some(ContentType::new(ELF)));
}

#[test]
fn elf_extension_fast_path() {
    let c = ElfClassifier::new();
    assert_eq!(c.classify(b"not elf content", "program.elf"), Some(ContentType::new(ELF)));
}

#[test]
fn elf_not_detected_for_text() {
    let c = ElfClassifier::new();
    assert!(c.classify(b"Hello, world!", "test.txt").is_none());
}

#[test]
fn elf_not_detected_empty() {
    let c = ElfClassifier::new();
    assert!(c.classify(b"", "test.bin").is_none());
}

#[test]
fn elf_too_short() {
    let c = ElfClassifier::new();
    assert!(c.classify(b"\x7fEL", "test.bin").is_none());
}

#[test]
fn elf_priority_is_33() {
    let c = ElfClassifier::new();
    assert_eq!(c.priority(), 33);
}

#[test]
fn elf_name_is_elf() {
    let c = ElfClassifier::new();
    assert_eq!(c.name(), "elf");
}

#[test]
fn elf_default_impl() {
    let c = ElfClassifier;
    assert_eq!(c.name(), "elf");
}

// === ZIP Classifier Tests ===

#[test]
fn zip_magic_detected() {
    let c = ZipClassifier::new();
    let data = b"PK\x03\x04\x14\x00\x00\x00\x00\x00";
    assert_eq!(c.classify(data, "unknown"), Some(ContentType::new(ZIP)));
}

#[test]
fn zip_extension_fast_path() {
    let c = ZipClassifier::new();
    assert_eq!(c.classify(b"not zip content", "archive.zip"), Some(ContentType::new(ZIP)));
}

#[test]
fn zip_jar_extension() {
    let c = ZipClassifier::new();
    assert_eq!(c.classify(b"data", "app.jar"), Some(ContentType::new(ZIP)));
}

#[test]
fn zip_docx_extension() {
    let c = ZipClassifier::new();
    assert_eq!(c.classify(b"data", "doc.docx"), Some(ContentType::new(ZIP)));
}

#[test]
fn zip_apk_extension() {
    let c = ZipClassifier::new();
    assert_eq!(c.classify(b"data", "app.apk"), Some(ContentType::new(ZIP)));
}

#[test]
fn zip_extension_case_insensitive() {
    let c = ZipClassifier::new();
    assert_eq!(c.classify(b"data", "archive.ZIP"), Some(ContentType::new(ZIP)));
}

#[test]
fn zip_not_detected_for_text() {
    let c = ZipClassifier::new();
    assert!(c.classify(b"Hello, world!", "test.txt").is_none());
}

#[test]
fn zip_not_detected_empty() {
    let c = ZipClassifier::new();
    assert!(c.classify(b"", "test.bin").is_none());
}

#[test]
fn zip_too_short() {
    let c = ZipClassifier::new();
    assert!(c.classify(b"PK\x03", "test.bin").is_none());
}

#[test]
fn zip_priority_is_31() {
    let c = ZipClassifier::new();
    assert_eq!(c.priority(), 31);
}

#[test]
fn zip_name_is_zip() {
    let c = ZipClassifier::new();
    assert_eq!(c.name(), "zip");
}

#[test]
fn zip_default_impl() {
    let c = ZipClassifier;
    assert_eq!(c.name(), "zip");
}

// === has_extension tests ===

#[test]
fn has_extension_tests() {
    assert!(has_extension("test.zip", ZIP_EXTENSIONS));
    assert!(has_extension("test.ZIP", ZIP_EXTENSIONS));
    assert!(has_extension("test.jar", ZIP_EXTENSIONS));
    assert!(has_extension("test.elf", ELF_EXTENSIONS));
    assert!(!has_extension("test.txt", ZIP_EXTENSIONS));
    assert!(!has_extension("Makefile", ELF_EXTENSIONS));
}

#[test]
fn no_extension_not_detected() {
    let elf_c = ElfClassifier::new();
    let zip_c = ZipClassifier::new();
    assert!(elf_c.classify(b"data", "Makefile").is_none());
    assert!(zip_c.classify(b"data", "Makefile").is_none());
}
