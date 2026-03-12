use super::*;

use std::{ops::Range, sync::Arc};

use crate::{Annotation, HighlightCategory, SyntaxEdit};

/// A minimal test driver for unit tests.
struct TestDriver {
    language: String,
    parsed: bool,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl TestDriver {
    fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
            parsed: false,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SyntaxDriver for TestDriver {
    fn language(&self) -> &str {
        &self.language
    }

    fn parse(&mut self, _content: &str) {
        self.parsed = true;
    }

    fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {
        // No-op
    }

    fn highlights(&self, byte_range: Range<usize>) -> Vec<Annotation> {
        if self.parsed {
            vec![Annotation::new(
                byte_range.start,
                byte_range.end,
                HighlightCategory::new("comment"),
            )]
        } else {
            Vec::new()
        }
    }

    fn is_parsed(&self) -> bool {
        self.parsed
    }
}

/// A minimal test factory.
struct TestFactory;

#[cfg_attr(coverage_nightly, coverage(off))]
impl SyntaxDriverFactory for TestFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id == "rust" {
            Some(Box::new(TestDriver::new("rust")))
        } else {
            None
        }
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["rust"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "rust"
    }
}

fn buffer_id(n: usize) -> BufferId {
    BufferId::from_raw(n)
}

#[test]
fn test_syntax_session_state_new() {
    let state = SyntaxSessionState::new();
    assert!(state.is_empty());
    assert!(state.factory().is_none());
}

#[test]
fn test_syntax_session_state_session_extension() {
    let state = SyntaxSessionState::create();
    assert!(state.is_empty());
    assert!(state.factory().is_none());
}

#[test]
fn test_set_and_get() {
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);

    assert!(state.get(id).is_none());

    state.set(id, Box::new(TestDriver::new("rust")));

    assert!(state.get(id).is_some());
    assert_eq!(state.get(id).unwrap().language(), "rust");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_mut() {
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);

    state.set(id, Box::new(TestDriver::new("rust")));

    // Parse via mutable reference
    if let Some(driver) = state.get_mut(id) {
        assert!(!driver.is_parsed());
        driver.parse("fn main() {}");
        assert!(driver.is_parsed());
    }

    // Verify parse persisted
    assert!(state.get(id).unwrap().is_parsed());
}

#[test]
fn test_remove() {
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);

    state.set(id, Box::new(TestDriver::new("rust")));
    assert!(state.has_driver(id));

    let removed = state.remove(id);
    assert!(removed.is_some());
    assert!(!state.has_driver(id));
    assert!(state.get(id).is_none());
}

#[test]
fn test_set_factory() {
    let mut state = SyntaxSessionState::new();
    assert!(state.factory().is_none());

    state.set_factory(Arc::new(TestFactory));
    assert!(state.factory().is_some());
    assert!(state.factory().unwrap().supports("rust"));
}

#[test]
fn test_ensure_driver_with_factory() {
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);

    state.set_factory(Arc::new(TestFactory));

    // Should create driver via factory
    assert!(state.ensure_driver(id, "rust", "fn main() {}"));
    assert!(state.get(id).unwrap().is_parsed());

    // Should reuse existing driver
    assert!(state.ensure_driver(id, "rust", ""));
}

#[test]
fn test_ensure_driver_unsupported_language() {
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);

    state.set_factory(Arc::new(TestFactory));

    // Factory doesn't support python
    assert!(!state.ensure_driver(id, "python", "def foo(): pass"));
}

#[test]
fn test_ensure_driver_no_factory() {
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);

    // No factory set
    assert!(!state.ensure_driver(id, "rust", "fn main() {}"));
}

#[test]
fn test_multiple_buffers() {
    let mut state = SyntaxSessionState::new();
    let id1 = buffer_id(1);
    let id2 = buffer_id(2);

    state.set(id1, Box::new(TestDriver::new("rust")));
    state.set(id2, Box::new(TestDriver::new("python")));

    assert_eq!(state.len(), 2);
    assert_eq!(state.get(id1).unwrap().language(), "rust");
    assert_eq!(state.get(id2).unwrap().language(), "python");
}

#[test]
fn test_clear() {
    let mut state = SyntaxSessionState::new();
    state.set(buffer_id(1), Box::new(TestDriver::new("rust")));
    state.set(buffer_id(2), Box::new(TestDriver::new("python")));

    assert_eq!(state.len(), 2);
    state.clear();
    assert!(state.is_empty());
}

#[test]
fn test_debug_impl() {
    let mut state = SyntaxSessionState::new();
    state.set(buffer_id(1), Box::new(TestDriver::new("rust")));

    let debug = format!("{state:?}");
    assert!(debug.contains("SyntaxSessionState"));
    assert!(debug.contains("buffer_count"));
}

#[test]
fn test_get_folds_via_driver() {
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);
    state.set(id, Box::new(TestDriver::new("rust")));

    // TestDriver returns empty folds (default), but access works
    let folds = state.get(id).map_or_else(Vec::new, SyntaxDriver::folds);
    assert!(folds.is_empty());
}

#[test]
fn test_get_folds_no_driver() {
    let state = SyntaxSessionState::new();
    let id = buffer_id(1);

    // No driver: get() returns None, graceful
    let folds = state.get(id).map_or_else(Vec::new, SyntaxDriver::folds);
    assert!(folds.is_empty());
}

// ========================================================================
// Registry and ensure_driver_from_path Tests
// ========================================================================

fn make_test_registry() -> Arc<dyn crate::LanguageRegistry> {
    Arc::new(crate::DefaultLanguageRegistry::new(vec![
        crate::LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        crate::LanguageInfo::new("markdown", "Markdown").with_extensions(["md"]),
    ]))
}

#[test]
fn test_set_and_get_registry() {
    let mut state = SyntaxSessionState::new();
    assert!(state.registry().is_none());

    state.set_registry(make_test_registry());
    assert!(state.registry().is_some());
}

#[test]
fn test_detect_language_with_registry() {
    let mut state = SyntaxSessionState::new();
    state.set_registry(make_test_registry());

    assert_eq!(state.detect_language("main.rs"), Some("rust".to_string()));
    assert_eq!(state.detect_language("README.md"), Some("markdown".to_string()));
    assert_eq!(state.detect_language("file.txt"), None);
}

#[test]
fn test_detect_language_without_registry() {
    let state = SyntaxSessionState::new();
    assert_eq!(state.detect_language("main.rs"), None);
}

#[test]
fn test_ensure_driver_from_path_creates_driver() {
    let mut state = SyntaxSessionState::new();
    state.set_factory(Arc::new(TestFactory));
    state.set_registry(make_test_registry());

    let id = buffer_id(1);
    assert!(state.ensure_driver_from_path(id, "main.rs", "fn main() {}"));
    assert!(state.has_driver(id));
    assert_eq!(state.get(id).unwrap().language(), "rust");
}

#[test]
fn test_ensure_driver_from_path_existing_driver() {
    let mut state = SyntaxSessionState::new();
    state.set_factory(Arc::new(TestFactory));
    state.set_registry(make_test_registry());

    let id = buffer_id(1);
    state.set(id, Box::new(TestDriver::new("rust")));

    // Should return true (driver already exists)
    assert!(state.ensure_driver_from_path(id, "main.rs", ""));
}

#[test]
fn test_ensure_driver_from_path_unknown_extension() {
    let mut state = SyntaxSessionState::new();
    state.set_factory(Arc::new(TestFactory));
    state.set_registry(make_test_registry());

    let id = buffer_id(1);
    assert!(!state.ensure_driver_from_path(id, "file.txt", "hello"));
    assert!(!state.has_driver(id));
}

#[test]
fn test_ensure_driver_from_path_no_registry() {
    let mut state = SyntaxSessionState::new();
    state.set_factory(Arc::new(TestFactory));

    let id = buffer_id(1);
    assert!(!state.ensure_driver_from_path(id, "main.rs", "fn main() {}"));
}

#[test]
fn test_ensure_driver_from_path_no_factory() {
    let mut state = SyntaxSessionState::new();
    state.set_registry(make_test_registry());

    let id = buffer_id(1);
    assert!(!state.ensure_driver_from_path(id, "main.rs", "fn main() {}"));
}

#[test]
fn test_ensure_driver_from_path_unsupported_language() {
    let mut state = SyntaxSessionState::new();
    state.set_factory(Arc::new(TestFactory)); // TestFactory only supports "rust"
    state.set_registry(make_test_registry());

    let id = buffer_id(1);
    // Markdown detected but factory doesn't support it
    assert!(!state.ensure_driver_from_path(id, "README.md", "# Hello"));
}

#[test]
fn test_debug_with_registry() {
    let mut state = SyntaxSessionState::new();
    state.set_registry(make_test_registry());

    let debug = format!("{state:?}");
    assert!(debug.contains("has_registry"));
}
