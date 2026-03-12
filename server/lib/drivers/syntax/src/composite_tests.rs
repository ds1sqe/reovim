use super::*;

use std::ops::Range;

use crate::{Annotation, HighlightCategory, SyntaxEdit};

struct StubDriver {
    language: &'static str,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SyntaxDriver for StubDriver {
    fn language(&self) -> &str {
        self.language
    }

    fn parse(&mut self, _content: &str) {}

    fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {}

    fn highlights(&self, _byte_range: Range<usize>) -> Vec<Annotation> {
        vec![Annotation::new(0, 1, HighlightCategory::new("comment"))]
    }

    fn is_parsed(&self) -> bool {
        true
    }
}

struct SingleFactory {
    language: &'static str,
}

impl SyntaxDriverFactory for SingleFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id == self.language {
            Some(Box::new(StubDriver {
                language: self.language,
            }))
        } else {
            None
        }
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec![self.language]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == self.language
    }
}

#[test]
fn test_empty_composite() {
    let composite = CompositeFactory::new(vec![]);
    assert_eq!(composite.factory_count(), 0);
    assert!(composite.supported_languages().is_empty());
    assert!(!composite.supports("rust"));
    assert!(composite.create("rust").is_none());
}

#[test]
fn test_single_factory() {
    let composite = CompositeFactory::new(vec![Arc::new(SingleFactory { language: "rust" })]);

    assert_eq!(composite.factory_count(), 1);
    assert!(composite.supports("rust"));
    assert!(!composite.supports("python"));

    let driver = composite.create("rust");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "rust");

    assert!(composite.create("python").is_none());
}

#[test]
fn test_multiple_factories_routing() {
    let composite = CompositeFactory::new(vec![
        Arc::new(SingleFactory { language: "rust" }),
        Arc::new(SingleFactory {
            language: "markdown",
        }),
    ]);

    assert_eq!(composite.factory_count(), 2);
    assert!(composite.supports("rust"));
    assert!(composite.supports("markdown"));
    assert!(!composite.supports("python"));

    let rust = composite.create("rust").unwrap();
    assert_eq!(rust.language(), "rust");

    let md = composite.create("markdown").unwrap();
    assert_eq!(md.language(), "markdown");

    assert!(composite.create("python").is_none());
}

#[test]
fn test_supported_languages_combines() {
    let composite = CompositeFactory::new(vec![
        Arc::new(SingleFactory { language: "rust" }),
        Arc::new(SingleFactory {
            language: "markdown",
        }),
    ]);

    let langs = composite.supported_languages();
    assert_eq!(langs.len(), 2);
    assert!(langs.contains(&"rust"));
    assert!(langs.contains(&"markdown"));
}

#[test]
fn test_supported_languages_deduplicates() {
    let composite = CompositeFactory::new(vec![
        Arc::new(SingleFactory { language: "rust" }),
        Arc::new(SingleFactory { language: "rust" }),
    ]);

    let langs = composite.supported_languages();
    assert_eq!(langs.len(), 1);
    assert!(langs.contains(&"rust"));
}

#[test]
fn test_create_returns_first_match() {
    // Both factories support "rust" — first one wins
    let composite = CompositeFactory::new(vec![
        Arc::new(SingleFactory { language: "rust" }),
        Arc::new(SingleFactory { language: "rust" }),
    ]);

    let driver = composite.create("rust");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "rust");
}

#[test]
fn test_debug_impl() {
    let composite = CompositeFactory::new(vec![
        Arc::new(SingleFactory { language: "rust" }),
        Arc::new(SingleFactory {
            language: "markdown",
        }),
    ]);

    let debug = format!("{composite:?}");
    assert!(debug.contains("CompositeFactory"));
    assert!(debug.contains("factory_count"));
    assert!(debug.contains("languages"));
}

#[test]
fn test_debug_empty() {
    let composite = CompositeFactory::new(vec![]);
    let debug = format!("{composite:?}");
    assert!(debug.contains("CompositeFactory"));
    assert!(debug.contains('0'));
}
