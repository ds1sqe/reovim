use super::*;

struct TestProvider {
    decorations: Vec<Decoration>,
    valid: bool,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl DecorationProvider for TestProvider {
    fn name(&self) -> &'static str {
        "test"
    }

    fn group(&self) -> DecorationGroup {
        DecorationGroup::Language
    }

    fn decorations_for_range(&self, start_line: u32, end_line: u32) -> Vec<Decoration> {
        self.decorations
            .iter()
            .filter(|d| {
                let start = d.start_line();
                let end = d.end_line();
                // Check if decoration overlaps with requested range
                start <= end_line && end >= start_line
            })
            .cloned()
            .collect()
    }

    fn refresh(&mut self, _content: &str) {
        self.valid = true;
    }

    fn is_valid(&self) -> bool {
        self.valid
    }
}

#[test]
fn test_provider_basic() {
    let provider = TestProvider {
        decorations: vec![],
        valid: true,
    };

    assert_eq!(provider.name(), "test");
    assert_eq!(provider.group(), DecorationGroup::Language);
    assert!(provider.is_valid());
}

#[test]
fn test_provider_decorations_for_range() {
    use super::super::types::Span;

    let provider = TestProvider {
        decorations: vec![
            Decoration::conceal(Span::line(5, 0, 10), "a", None),
            Decoration::conceal(Span::line(10, 0, 10), "b", None),
            Decoration::conceal(Span::line(15, 0, 10), "c", None),
        ],
        valid: true,
    };

    // Range 0-4 should return nothing
    let result = provider.decorations_for_range(0, 4);
    assert_eq!(result.len(), 0);

    // Range 5-10 should return first two
    let result = provider.decorations_for_range(5, 10);
    assert_eq!(result.len(), 2);

    // Range 10-20 should return last two
    let result = provider.decorations_for_range(10, 20);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_provider_refresh() {
    let mut provider = TestProvider {
        decorations: vec![],
        valid: false,
    };

    assert!(!provider.is_valid());
    provider.refresh("some content");
    assert!(provider.is_valid());
}

#[test]
fn test_provider_default_is_valid() {
    // The default implementation of is_valid returns true
    struct MinimalProvider;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl DecorationProvider for MinimalProvider {
        fn name(&self) -> &'static str {
            "minimal"
        }
        fn group(&self) -> DecorationGroup {
            DecorationGroup::Language
        }
        fn decorations_for_range(&self, _start_line: u32, _end_line: u32) -> Vec<Decoration> {
            vec![]
        }
        fn refresh(&mut self, _content: &str) {}
    }

    let provider = MinimalProvider;
    assert!(provider.is_valid());
}

#[test]
fn test_factory_default_supported_languages() {
    struct MinimalFactory;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl DecorationProviderFactory for MinimalFactory {
        #[allow(clippy::unnecessary_literal_bound)]
        fn name(&self) -> &str {
            "minimal"
        }
        fn create(&self, _language_id: &str) -> Option<Box<dyn DecorationProvider>> {
            None
        }
    }

    let factory = MinimalFactory;
    assert!(factory.supported_languages().is_empty());
}
