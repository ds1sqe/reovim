use reovim_driver_syntax::SyntaxDriverFactory;

use super::*;

#[test]
fn test_factory_creation() {
    let factory = CSyntaxFactory::new();
    assert!(factory.supports("c"));
    assert!(!factory.supports("rust"));
    assert_eq!(factory.supported_languages(), vec!["c"]);
}

#[test]
fn test_factory_default() {
    let factory = CSyntaxFactory::default();
    assert!(factory.supports("c"));
    assert!(!factory.supports("python"));
}

#[test]
fn test_factory_folds_query_accessor() {
    let factory = CSyntaxFactory::new();
    let folds_query = factory.folds_query();
    let _clone = Arc::clone(folds_query);
}

#[test]
fn test_create_driver() {
    let factory = CSyntaxFactory::new();

    let driver = factory.create("c");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "c");

    let driver = factory.create("rust");
    assert!(driver.is_none());
}

#[test]
fn test_factory_rejects_unknown_language() {
    let factory = CSyntaxFactory::new();
    assert!(factory.create("rust").is_none());
    assert!(factory.create("python").is_none());
    assert!(factory.create("").is_none());
    assert!(factory.create("C").is_none());
}

#[test]
fn test_driver_is_not_parsed_initially() {
    let factory = CSyntaxFactory::new();
    let driver = factory.create("c").unwrap();
    assert!(!driver.is_parsed());
}

#[test]
fn test_driver_parse_and_highlights() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("int main() { return 0; }");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..100);
    assert!(!highlights.is_empty(), "Expected highlights for C code");
}

#[test]
fn test_highlights_contain_keyword() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("int main() { return 0; }");
    let highlights = driver.highlights(0..100);

    let keyword_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("keyword"));

    assert!(keyword_highlight.is_some(), "Expected keyword highlight");
}

#[test]
fn test_highlights_contain_type() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("int x = 1;");
    let highlights = driver.highlights(0..100);

    let type_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("type"));

    assert!(type_highlight.is_some(), "Expected type highlight for int");
}

#[test]
fn test_highlights_contain_string() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse(r#"char *s = "hello";"#);
    let highlights = driver.highlights(0..100);

    let string_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("string"));

    assert!(string_highlight.is_some(), "Expected string highlight");
}

#[test]
fn test_highlights_contain_number() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("int x = 42;");
    let highlights = driver.highlights(0..100);

    let number_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("number"));

    assert!(number_highlight.is_some(), "Expected number highlight");
}

#[test]
fn test_highlights_contain_comment() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("// this is a comment\nint x = 1;");
    let highlights = driver.highlights(0..100);

    let comment_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("comment"));

    assert!(comment_highlight.is_some(), "Expected comment highlight");
}

#[test]
fn test_highlights_contain_function() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("int main() { return 0; }");
    let highlights = driver.highlights(0..100);

    let fn_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("function"));

    assert!(fn_highlight.is_some(), "Expected function highlight");
}

#[test]
fn test_folds() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    let code = "int main() {\n    int x = 1;\n    int y = 2;\n    return x + y;\n}";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected folds for C function body");
}

#[test]
fn test_folds_struct() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    let code = "struct Point {\n    int x;\n    int y;\n};";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected folds for struct definition");
}

#[test]
fn test_folds_empty_file() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("");
    let folds = driver.folds();
    assert!(folds.is_empty(), "Empty file should have no folds");
}

#[test]
fn test_empty_file() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..0);
    assert!(highlights.is_empty());
}

#[test]
fn test_module_metadata() {
    let module = TreesitterCModule::new();
    assert_eq!(module.id(), ModuleId::new("treesitter-c"));
    assert_eq!(module.name(), "Treesitter C");
    assert_eq!(module.version(), Version::new(0, 10, 0));
}

#[test]
fn test_module_default() {
    fn takes_default<T: Default>(val: T) -> T {
        drop(val);
        T::default()
    }
    let module = takes_default(TreesitterCModule::new());
    assert_eq!(module.id(), ModuleId::new("treesitter-c"));
}

#[test]
fn test_module_exit() {
    let mut module = TreesitterCModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_factory_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CSyntaxFactory>();
}

#[test]
fn test_driver_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
    let factory = CSyntaxFactory::new();
    let driver = factory.create("c").unwrap();
    assert_send_sync(&*driver);
}

#[test]
fn test_multiple_factory_instances() {
    let factory1 = CSyntaxFactory::new();
    let factory2 = CSyntaxFactory::new();

    let mut driver1 = factory1.create("c").unwrap();
    let mut driver2 = factory2.create("c").unwrap();

    driver1.parse("int main() { return 0; }");
    driver2.parse("int x = 1;");

    assert!(driver1.is_parsed());
    assert!(driver2.is_parsed());

    let h1 = driver1.highlights(0..100);
    let h2 = driver2.highlights(0..100);

    assert!(!h1.is_empty());
    assert!(!h2.is_empty());
}

#[test]
fn test_no_injections() {
    let factory = CSyntaxFactory::new();
    let mut driver = factory.create("c").unwrap();

    driver.parse("int main() { return 0; }");
    let injections = driver.injections();
    assert!(injections.is_empty());
}
