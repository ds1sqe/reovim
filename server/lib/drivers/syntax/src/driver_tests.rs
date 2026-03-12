use {super::*, crate::HighlightCategory};

/// A minimal test implementation of `SyntaxDriver`.
struct TestDriver {
    language: String,
    parsed: bool,
}

impl TestDriver {
    fn new(language: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            parsed: false,
        }
    }
}

impl SyntaxDriver for TestDriver {
    fn language(&self) -> &str {
        &self.language
    }

    fn parse(&mut self, _content: &str) {
        self.parsed = true;
    }

    fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {
        // No-op for test
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

#[test]
fn test_syntax_driver_basic() {
    let mut driver = TestDriver::new("test");

    assert_eq!(driver.language(), "test");
    assert!(!driver.is_parsed());

    driver.parse("some content");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..10);
    assert_eq!(highlights.len(), 1);
    assert_eq!(highlights[0].start_byte, 0);
    assert_eq!(highlights[0].end_byte, 10);
    assert_eq!(highlights[0].category.as_str(), "comment");
}

#[test]
fn test_syntax_driver_defaults() {
    let driver = TestDriver::new("test");

    // Default implementations return empty/None
    assert!(driver.decorations(0..100).is_empty());
    assert!(driver.injections().is_empty());
    assert!(driver.folds().is_empty());
    assert_eq!(driver.indent_for(0), None);
}

#[test]
fn test_syntax_driver_update() {
    let mut driver = TestDriver::new("test");
    driver.parse("initial");
    assert!(driver.is_parsed());

    let edit = SyntaxEdit::insert(0, 0, 0, 10, 0, 10);
    driver.update("updated content", &edit);
    // Still parsed after update
    assert!(driver.is_parsed());
}

#[test]
fn test_syntax_driver_highlights_before_parse() {
    let driver = TestDriver::new("test");

    // Before parsing, should return empty
    let highlights = driver.highlights(0..100);
    assert!(highlights.is_empty());
}

#[test]
fn test_syntax_driver_highlights_after_parse() {
    let mut driver = TestDriver::new("test");
    driver.parse("some content");

    let highlights = driver.highlights(5..15);
    assert_eq!(highlights.len(), 1);
    assert_eq!(highlights[0].start_byte, 5);
    assert_eq!(highlights[0].end_byte, 15);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_syntax_driver_is_object_safe() {
    fn _accepts_ref(_: &dyn SyntaxDriver) {}
    fn _accepts_box(_: Box<dyn SyntaxDriver>) {}
}

#[test]
fn test_syntax_driver_indent_for_multiple_lines() {
    let driver = TestDriver::new("test");
    assert_eq!(driver.indent_for(0), None);
    assert_eq!(driver.indent_for(100), None);
    assert_eq!(driver.indent_for(usize::MAX), None);
}
