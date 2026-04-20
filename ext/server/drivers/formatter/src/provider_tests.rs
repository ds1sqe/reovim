use std::path::Path;

use {super::*, crate::error::FormatError};

struct DummyFormatter;

impl FormatterProvider for DummyFormatter {
    fn format(&self, content: &str, _path: &Path) -> Result<String, FormatError> {
        Ok(content.to_uppercase())
    }

    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "dummy"
    }
}

#[test]
fn test_format() {
    let fmt = DummyFormatter;
    let result = fmt.format("hello", Path::new("test.rs")).unwrap();
    assert_eq!(result, "HELLO");
}

#[test]
fn test_name() {
    let fmt = DummyFormatter;
    assert_eq!(fmt.name(), "dummy");
}

#[test]
fn test_supports_range_default() {
    let fmt = DummyFormatter;
    assert!(!fmt.supports_range());
}

#[test]
fn test_format_range_default_delegates_to_format() {
    let fmt = DummyFormatter;
    let result = fmt
        .format_range("hello", Path::new("test.rs"), 0, 5)
        .unwrap();
    assert_eq!(result, "HELLO");
}

#[test]
fn test_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DummyFormatter>();
}
