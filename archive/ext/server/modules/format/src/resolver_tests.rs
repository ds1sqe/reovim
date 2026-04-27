use std::{path::Path, sync::Arc};

use {
    reovim_driver_formatter::{FormatError, FormatterProvider, FormatterRegistry},
    reovim_kernel::api::v1::ServiceRegistry,
};

use super::*;

// ============================================================================
// detect_filetype tests
// ============================================================================

#[test]
fn test_detect_filetype_rust() {
    assert_eq!(detect_filetype("main.rs"), "rust");
}

#[test]
fn test_detect_filetype_python() {
    assert_eq!(detect_filetype("script.py"), "python");
}

#[test]
fn test_detect_filetype_unknown() {
    assert_eq!(detect_filetype("Makefile"), "unknown");
}

// ============================================================================
// resolve_and_format tests
// ============================================================================

struct UpperFormatter;

impl FormatterProvider for UpperFormatter {
    fn format(&self, content: &str, _path: &Path) -> Result<String, FormatError> {
        Ok(content.to_uppercase())
    }

    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "upper"
    }
}

struct FailingFormatter;

impl FormatterProvider for FailingFormatter {
    fn format(&self, _content: &str, _path: &Path) -> Result<String, FormatError> {
        Err(FormatError::CommandNotFound("nonexistent".to_string()))
    }

    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "failing"
    }
}

#[test]
fn test_resolve_external_formatter_found() {
    let services = Arc::new(ServiceRegistry::new());
    let mut registry = FormatterRegistry::new();
    registry.register("rust", Box::new(UpperFormatter));
    services.register(Arc::new(registry));

    let result = resolve_and_format("hello", "main.rs", "rust", &services);
    assert_eq!(result, Some("HELLO".to_string()));
}

#[test]
fn test_resolve_no_formatter() {
    let services = Arc::new(ServiceRegistry::new());

    let result = resolve_and_format("hello", "main.rs", "rust", &services);
    assert!(result.is_none());
}

#[test]
fn test_resolve_external_formatter_wrong_filetype() {
    let services = Arc::new(ServiceRegistry::new());
    let mut registry = FormatterRegistry::new();
    registry.register("python", Box::new(UpperFormatter));
    services.register(Arc::new(registry));

    // Requesting rust, but only python registered
    let result = resolve_and_format("hello", "main.rs", "rust", &services);
    assert!(result.is_none());
}

#[test]
fn test_resolve_external_formatter_fails_falls_through() {
    let services = Arc::new(ServiceRegistry::new());
    let mut registry = FormatterRegistry::new();
    registry.register("rust", Box::new(FailingFormatter));
    services.register(Arc::new(registry));

    // External fails, no LSP available -> None
    let result = resolve_and_format("hello", "main.rs", "rust", &services);
    assert!(result.is_none());
}

#[test]
fn test_resolve_empty_registry() {
    let services = Arc::new(ServiceRegistry::new());
    services.register(Arc::new(FormatterRegistry::new()));

    let result = resolve_and_format("hello", "main.rs", "rust", &services);
    assert!(result.is_none());
}
