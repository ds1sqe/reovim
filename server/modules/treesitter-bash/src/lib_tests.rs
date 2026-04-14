use reovim_driver_text_syntax::SyntaxDriverFactory;

use super::*;

#[test]
fn test_factory_creation() {
    let factory = BashSyntaxFactory::new();
    assert!(factory.supports("bash"));
    assert!(!factory.supports("rust"));
    assert_eq!(factory.supported_languages(), vec!["bash"]);
}

#[test]
fn test_factory_default() {
    let factory = BashSyntaxFactory::default();
    assert!(factory.supports("bash"));
    assert!(!factory.supports("python"));
}

#[test]
fn test_factory_folds_query_accessor() {
    let factory = BashSyntaxFactory::new();
    let folds_query = factory.folds_query();
    let _clone = Arc::clone(folds_query);
}

#[test]
fn test_create_driver() {
    let factory = BashSyntaxFactory::new();

    let driver = factory.create("bash");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "bash");

    let driver = factory.create("rust");
    assert!(driver.is_none());
}

#[test]
fn test_factory_rejects_unknown_language() {
    let factory = BashSyntaxFactory::new();
    assert!(factory.create("rust").is_none());
    assert!(factory.create("python").is_none());
    assert!(factory.create("").is_none());
    assert!(factory.create("Bash").is_none());
}

#[test]
fn test_driver_is_not_parsed_initially() {
    let factory = BashSyntaxFactory::new();
    let driver = factory.create("bash").unwrap();
    assert!(!driver.is_parsed());
}

#[test]
fn test_driver_parse_and_highlights() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("#!/bin/bash\necho \"hello\"");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..100);
    assert!(!highlights.is_empty(), "Expected highlights for Bash code");
}

#[test]
fn test_highlights_contain_string() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("echo \"hello world\"");
    let highlights = driver.highlights(0..100);

    let string_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("string"));

    assert!(string_highlight.is_some(), "Expected string highlight");
}

#[test]
fn test_highlights_contain_comment() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("# this is a comment\necho hello");
    let highlights = driver.highlights(0..100);

    let comment_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("comment"));

    assert!(comment_highlight.is_some(), "Expected comment highlight");
}

#[test]
fn test_highlights_contain_function() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("echo hello");
    let highlights = driver.highlights(0..100);

    let fn_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("function"));

    assert!(fn_highlight.is_some(), "Expected function highlight for command");
}

#[test]
fn test_highlights_contain_keyword() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("if true; then\n    echo yes\nfi");
    let highlights = driver.highlights(0..100);

    let keyword_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("keyword"));

    assert!(keyword_highlight.is_some(), "Expected keyword highlight");
}

#[test]
fn test_highlights_contain_variable() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("NAME=\"test\"\necho $NAME");
    let highlights = driver.highlights(0..100);

    let var_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("variable"));

    assert!(var_highlight.is_some(), "Expected variable highlight");
}

#[test]
fn test_folds() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    let code = "my_func() {\n    echo \"hello\"\n    echo \"world\"\n}";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected folds for Bash function definition");
}

#[test]
fn test_folds_if_block() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    let code = "if [ -f /tmp/test ]; then\n    echo exists\n    echo done\nfi";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected folds for if block");
}

#[test]
fn test_folds_empty_file() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("");
    let folds = driver.folds();
    assert!(folds.is_empty(), "Empty file should have no folds");
}

#[test]
fn test_empty_file() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..0);
    assert!(highlights.is_empty());
}

#[test]
fn test_module_metadata() {
    let module = TreesitterBashModule::new();
    assert_eq!(module.id(), ModuleId::new("treesitter-bash"));
    assert_eq!(module.name(), "Treesitter Bash");
    assert_eq!(module.version(), Version::new(0, 10, 0));
}

#[test]
fn test_module_default() {
    fn takes_default<T: Default>(val: T) -> T {
        drop(val);
        T::default()
    }
    let module = takes_default(TreesitterBashModule::new());
    assert_eq!(module.id(), ModuleId::new("treesitter-bash"));
}

#[test]
fn test_module_exit() {
    let mut module = TreesitterBashModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_factory_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BashSyntaxFactory>();
}

#[test]
fn test_driver_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
    let factory = BashSyntaxFactory::new();
    let driver = factory.create("bash").unwrap();
    assert_send_sync(&*driver);
}

#[test]
fn test_multiple_factory_instances() {
    let factory1 = BashSyntaxFactory::new();
    let factory2 = BashSyntaxFactory::new();

    let mut driver1 = factory1.create("bash").unwrap();
    let mut driver2 = factory2.create("bash").unwrap();

    driver1.parse("echo hello");
    driver2.parse("echo world");

    assert!(driver1.is_parsed());
    assert!(driver2.is_parsed());

    let h1 = driver1.highlights(0..100);
    let h2 = driver2.highlights(0..100);

    assert!(!h1.is_empty());
    assert!(!h2.is_empty());
}

#[test]
fn test_no_injections() {
    let factory = BashSyntaxFactory::new();
    let mut driver = factory.create("bash").unwrap();

    driver.parse("echo hello");
    let injections = driver.injections();
    assert!(injections.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_init() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let mut module = TreesitterBashModule::new();
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
    let factory = syntax_store.find("bash");
    assert!(factory.is_some(), "Bash factory should be available after module init");
}
