use reovim_driver_text_syntax::SyntaxDriverFactory;

use super::*;

#[test]
fn test_factory_creation() {
    let factory = GoSyntaxFactory::new();
    assert!(factory.supports("go"));
    assert!(!factory.supports("rust"));
    assert_eq!(factory.supported_languages(), vec!["go"]);
}

#[test]
fn test_factory_default() {
    let factory = GoSyntaxFactory::default();
    assert!(factory.supports("go"));
}

#[test]
fn test_factory_supports_negative_cases() {
    let factory = GoSyntaxFactory::new();
    assert!(!factory.supports(""));
    assert!(!factory.supports("Go"));
    assert!(!factory.supports("GO"));
    assert!(!factory.supports("javascript"));
    assert!(!factory.supports("rust"));
}

#[test]
fn test_factory_folds_query_accessor() {
    let factory = GoSyntaxFactory::new();
    let folds_query = factory.folds_query();
    let _clone = Arc::clone(folds_query);
}

#[test]
fn test_create_driver() {
    let factory = GoSyntaxFactory::new();
    let driver = factory.create("go");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "go");
}

#[test]
fn test_factory_rejects_unknown_language() {
    let factory = GoSyntaxFactory::new();
    assert!(factory.create("rust").is_none());
    assert!(factory.create("javascript").is_none());
    assert!(factory.create("").is_none());
    assert!(factory.create("Go").is_none());
}

#[test]
fn test_driver_is_not_parsed_initially() {
    let factory = GoSyntaxFactory::new();
    let driver = factory.create("go").unwrap();
    assert!(!driver.is_parsed());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_driver_parse_and_highlights() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    let code = "package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}";
    driver.parse(code);
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..code.len());
    assert!(!highlights.is_empty(), "Expected highlights for Go code");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_highlights_contain_keyword() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("package main\n\nfunc main() {}");
    let highlights = driver.highlights(0..100);

    let keyword_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("keyword"));

    assert!(
        keyword_highlight.is_some(),
        "Expected keyword highlight for 'package' or 'func'"
    );
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_highlights_contain_function_name() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("package main\n\nfunc main() {}");
    let highlights = driver.highlights(0..100);

    let func_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("function"));

    assert!(func_highlight.is_some(), "Expected function highlight for 'main'");
}

#[test]
fn test_highlights_contain_string() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("package main\n\nvar s = \"hello\"");
    let highlights = driver.highlights(0..100);

    let string_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("string"));

    assert!(string_highlight.is_some(), "Expected string highlight");
}

#[test]
fn test_highlights_contain_number() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("package main\n\nvar x = 42");
    let highlights = driver.highlights(0..100);

    let number_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("number"));

    assert!(number_highlight.is_some(), "Expected number highlight");
}

#[test]
fn test_highlights_contain_comment() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("// comment\npackage main");
    let highlights = driver.highlights(0..100);

    let comment_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("comment"));

    assert!(comment_highlight.is_some(), "Expected comment highlight");
}

#[test]
fn test_highlights_contain_boolean() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("package main\n\nvar b = true");
    let highlights = driver.highlights(0..100);

    let bool_highlight = highlights.iter().find(|h| h.category.as_str() == "boolean");

    assert!(bool_highlight.is_some(), "Expected boolean highlight");
}

#[test]
fn test_empty_file() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..0);
    assert!(highlights.is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_folds_function() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    let code = "package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected fold for function body");
}

#[test]
fn test_folds_empty_file() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("");
    let folds = driver.folds();
    assert!(folds.is_empty(), "Empty file should have no folds");
}

#[test]
fn test_no_injections() {
    let factory = GoSyntaxFactory::new();
    let mut driver = factory.create("go").unwrap();

    driver.parse("package main\n\nfunc main() {}");
    let injections = driver.injections();
    assert!(injections.is_empty());
}

// ========================================================================
// Module Trait Tests
// ========================================================================

#[test]
fn test_module_metadata() {
    let module = TreesitterGoModule::new();
    assert_eq!(module.id(), ModuleId::new("treesitter-go"));
    assert_eq!(module.name(), "Treesitter Go");
    assert_eq!(module.version(), Version::new(0, 10, 0));
}

#[test]
fn test_module_default() {
    fn takes_default<T: Default>(val: T) -> T {
        drop(val);
        T::default()
    }
    let module = takes_default(TreesitterGoModule::new());
    assert_eq!(module.id(), ModuleId::new("treesitter-go"));
}

#[test]
fn test_module_exit() {
    let mut module = TreesitterGoModule::new();
    assert!(module.exit().is_ok());
}

// ========================================================================
// Send + Sync Tests
// ========================================================================

#[test]
fn test_factory_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<GoSyntaxFactory>();
}

#[test]
fn test_driver_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
    let factory = GoSyntaxFactory::new();
    let driver = factory.create("go").unwrap();
    assert_send_sync(&*driver);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_init() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let mut module = TreesitterGoModule::new();
    let kernel = reovim_kernel::api::v1::KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        std::path::PathBuf::from("/tmp/test-data"),
        std::path::PathBuf::from("/tmp/test-cache"),
    );
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify that the factory was registered
    let syntax_store = services.get_or_create::<SyntaxFactoryStore>();
    let factory = syntax_store.find("go");
    assert!(factory.is_some(), "Go factory should be available after module init");
}
