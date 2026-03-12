use reovim_driver_syntax::SyntaxDriverFactory;

use super::*;

// ========================================================================
// Factory Tests
// ========================================================================

#[test]
fn test_factory_creation() {
    let factory = TypeScriptSyntaxFactory::new();
    assert!(factory.supports("typescript"));
    assert!(factory.supports("typescriptreact"));
    assert!(!factory.supports("rust"));
    assert_eq!(factory.supported_languages(), vec!["typescript", "typescriptreact"]);
}

#[test]
fn test_factory_default() {
    let factory = TypeScriptSyntaxFactory::default();
    assert!(factory.supports("typescript"));
    assert!(factory.supports("typescriptreact"));
}

#[test]
fn test_factory_supports_negative_cases() {
    let factory = TypeScriptSyntaxFactory::new();
    assert!(!factory.supports(""));
    assert!(!factory.supports("TypeScript"));
    assert!(!factory.supports("TS"));
    assert!(!factory.supports("javascript"));
    assert!(!factory.supports("tsx")); // Must use "typescriptreact"
    assert!(!factory.supports("rust"));
}

#[test]
fn test_factory_folds_query_accessors() {
    let factory = TypeScriptSyntaxFactory::new();
    let folds_typescript = factory.folds_query_ts();
    let folds_react = factory.folds_query_tsx();
    let _clone_typescript = Arc::clone(folds_typescript);
    let _clone_react = Arc::clone(folds_react);
}

// ========================================================================
// TypeScript Driver Tests
// ========================================================================

#[test]
fn test_create_typescript_driver() {
    let factory = TypeScriptSyntaxFactory::new();
    let driver = factory.create("typescript");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "typescript");
}

#[test]
fn test_create_tsx_driver() {
    let factory = TypeScriptSyntaxFactory::new();
    let driver = factory.create("typescriptreact");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "typescriptreact");
}

#[test]
fn test_factory_rejects_unknown_language() {
    let factory = TypeScriptSyntaxFactory::new();
    assert!(factory.create("rust").is_none());
    assert!(factory.create("javascript").is_none());
    assert!(factory.create("").is_none());
    assert!(factory.create("TypeScript").is_none());
    assert!(factory.create("tsx").is_none());
}

#[test]
fn test_driver_is_not_parsed_initially() {
    let factory = TypeScriptSyntaxFactory::new();
    let driver = factory.create("typescript").unwrap();
    assert!(!driver.is_parsed());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_typescript_parse_and_highlights() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    let code = "function hello(): string { return \"hi\"; }";
    driver.parse(code);
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..code.len());
    assert!(!highlights.is_empty(), "Expected highlights for TypeScript code");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_tsx_parse_and_highlights() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescriptreact").unwrap();

    let code = "const App = () => <div>Hello</div>;";
    driver.parse(code);
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..code.len());
    assert!(!highlights.is_empty(), "Expected highlights for TSX code");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_highlights_contain_keyword() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("function hello(): string { return \"hi\"; }");
    let highlights = driver.highlights(0..100);

    let keyword_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("keyword"));

    assert!(keyword_highlight.is_some(), "Expected keyword highlight for 'function'");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_highlights_contain_function_name() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("function hello() {}");
    let highlights = driver.highlights(0..100);

    let func_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("function"));

    assert!(func_highlight.is_some(), "Expected function highlight for 'hello'");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_highlights_contain_type() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("let x: string = \"hi\";");
    let highlights = driver.highlights(0..100);

    let type_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("type"));

    assert!(type_highlight.is_some(), "Expected type highlight for 'string'");
}

#[test]
fn test_highlights_contain_string() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("let s = \"hello\";");
    let highlights = driver.highlights(0..100);

    let string_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("string"));

    assert!(string_highlight.is_some(), "Expected string highlight");
}

#[test]
fn test_highlights_contain_number() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("let x = 42;");
    let highlights = driver.highlights(0..100);

    let number_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("number"));

    assert!(number_highlight.is_some(), "Expected number highlight");
}

#[test]
fn test_highlights_contain_comment() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("// comment\nlet x = 1;");
    let highlights = driver.highlights(0..100);

    let comment_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("comment"));

    assert!(comment_highlight.is_some(), "Expected comment highlight");
}

#[test]
fn test_highlights_contain_boolean() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("let b = true;");
    let highlights = driver.highlights(0..100);

    let bool_highlight = highlights.iter().find(|h| h.category.as_str() == "boolean");

    assert!(bool_highlight.is_some(), "Expected boolean highlight");
}

#[test]
fn test_empty_file() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..0);
    assert!(highlights.is_empty());
}

// ========================================================================
// Fold Tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_folds_typescript_function() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    let code = "function hello(): string {\n  return \"hi\";\n}";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected fold for TypeScript function body");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_folds_tsx_function() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescriptreact").unwrap();

    let code = "function App() {\n  return <div>Hello</div>;\n}";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected fold for TSX function body");
}

#[test]
fn test_folds_empty_file() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("");
    let folds = driver.folds();
    assert!(folds.is_empty(), "Empty file should have no folds");
}

#[test]
fn test_no_injections() {
    let factory = TypeScriptSyntaxFactory::new();
    let mut driver = factory.create("typescript").unwrap();

    driver.parse("function hello(): string { return \"hi\"; }");
    let injections = driver.injections();
    assert!(injections.is_empty());
}

// ========================================================================
// Module Trait Tests
// ========================================================================

#[test]
fn test_module_metadata() {
    let module = TreesitterTypeScriptModule::new();
    assert_eq!(module.id(), ModuleId::new("treesitter-typescript"));
    assert_eq!(module.name(), "Treesitter TypeScript");
    assert_eq!(module.version(), Version::new(0, 10, 0));
}

#[test]
fn test_module_default() {
    fn takes_default<T: Default>(val: T) -> T {
        drop(val);
        T::default()
    }
    let module = takes_default(TreesitterTypeScriptModule::new());
    assert_eq!(module.id(), ModuleId::new("treesitter-typescript"));
}

#[test]
fn test_module_exit() {
    let mut module = TreesitterTypeScriptModule::new();
    assert!(module.exit().is_ok());
}

// ========================================================================
// Send + Sync Tests
// ========================================================================

#[test]
fn test_factory_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TypeScriptSyntaxFactory>();
}

#[test]
fn test_typescript_driver_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
    let factory = TypeScriptSyntaxFactory::new();
    let driver = factory.create("typescript").unwrap();
    assert_send_sync(&*driver);
}

#[test]
fn test_tsx_driver_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
    let factory = TypeScriptSyntaxFactory::new();
    let driver = factory.create("typescriptreact").unwrap();
    assert_send_sync(&*driver);
}
