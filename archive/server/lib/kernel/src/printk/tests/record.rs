use super::super::*;

#[test]
fn test_record_builder() {
    let record = Record::builder(Level::Info)
        .message("test message")
        .module_path("test::module")
        .file("test.rs")
        .line(42)
        .build();

    assert_eq!(record.level(), Level::Info);
    assert_eq!(record.message(), "test message");
    assert_eq!(record.module_path(), "test::module");
    assert_eq!(record.file(), "test.rs");
    assert_eq!(record.line(), 42);
}

#[test]
fn test_record_builder_defaults() {
    let record = Record::builder(Level::Error).build();

    assert_eq!(record.level(), Level::Error);
    assert_eq!(record.message(), "");
    assert_eq!(record.module_path(), "");
    assert_eq!(record.file(), "");
    assert_eq!(record.line(), 0);
}

#[test]
fn test_record_clone() {
    let original = Record::builder(Level::Warn).message("warning").build();

    let cloned = original.clone();

    assert_eq!(cloned.level(), original.level());
    assert_eq!(cloned.message(), original.message());
}

#[test]
fn test_record_debug() {
    let record = Record::builder(Level::Debug)
        .message("debug msg")
        .file("lib.rs")
        .line(10)
        .build();

    let debug_str = format!("{record:?}");
    assert!(debug_str.contains("Debug"));
    assert!(debug_str.contains("debug msg"));
    assert!(debug_str.contains("lib.rs"));
    assert!(debug_str.contains("10"));
}
