use std::path::Path;

use {super::*, crate::error::FormatError};

struct MockFormatter {
    name: &'static str,
}

impl FormatterProvider for MockFormatter {
    fn format(&self, content: &str, _path: &Path) -> Result<String, FormatError> {
        Ok(format!("formatted:{content}"))
    }

    fn name(&self) -> &str {
        self.name
    }
}

#[test]
fn test_new_is_empty() {
    let registry = FormatterRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_default_is_empty() {
    let registry = FormatterRegistry::default();
    assert!(registry.is_empty());
}

#[test]
fn test_register_and_get() {
    let mut registry = FormatterRegistry::new();
    registry.register("rust", Box::new(MockFormatter { name: "rustfmt" }));

    let fmt = registry.get("rust").unwrap();
    assert_eq!(fmt.name(), "rustfmt");
}

#[test]
fn test_get_nonexistent() {
    let registry = FormatterRegistry::new();
    assert!(registry.get("rust").is_none());
}

#[test]
fn test_has() {
    let mut registry = FormatterRegistry::new();
    registry.register("python", Box::new(MockFormatter { name: "black" }));

    assert!(registry.has("python"));
    assert!(!registry.has("rust"));
}

#[test]
fn test_len() {
    let mut registry = FormatterRegistry::new();
    assert_eq!(registry.len(), 0);

    registry.register("rust", Box::new(MockFormatter { name: "rustfmt" }));
    assert_eq!(registry.len(), 1);

    registry.register("python", Box::new(MockFormatter { name: "black" }));
    assert_eq!(registry.len(), 2);
}

#[test]
fn test_register_replaces_existing() {
    let mut registry = FormatterRegistry::new();
    registry.register("rust", Box::new(MockFormatter { name: "rustfmt" }));
    registry.register("rust", Box::new(MockFormatter { name: "rustfmt2" }));

    assert_eq!(registry.len(), 1);
    assert_eq!(registry.get("rust").unwrap().name(), "rustfmt2");
}

#[test]
fn test_format_via_registry() {
    let mut registry = FormatterRegistry::new();
    registry.register("rust", Box::new(MockFormatter { name: "rustfmt" }));

    let fmt = registry.get("rust").unwrap();
    let result = fmt.format("hello", Path::new("test.rs")).unwrap();
    assert_eq!(result, "formatted:hello");
}

#[test]
fn test_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<FormatterRegistry>();
}
