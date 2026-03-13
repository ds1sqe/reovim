use reovim_driver_syntax::SyntaxDriverFactory;

use super::*;

#[test]
fn test_factory_creation() {
    let factory = PythonSyntaxFactory::new();
    assert!(factory.supports("python"));
    assert!(!factory.supports("rust"));
    assert_eq!(factory.supported_languages(), vec!["python"]);
}

#[test]
fn test_factory_default() {
    let factory = PythonSyntaxFactory::default();
    assert!(factory.supports("python"));
    assert!(!factory.supports("markdown"));
}

#[test]
fn test_factory_folds_query_accessor() {
    let factory = PythonSyntaxFactory::new();
    let folds_query = factory.folds_query();
    let _clone = Arc::clone(folds_query);
}

#[test]
fn test_create_driver() {
    let factory = PythonSyntaxFactory::new();

    let driver = factory.create("python");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "python");

    let driver = factory.create("rust");
    assert!(driver.is_none());
}

#[test]
fn test_factory_rejects_unknown_language() {
    let factory = PythonSyntaxFactory::new();
    assert!(factory.create("rust").is_none());
    assert!(factory.create("c").is_none());
    assert!(factory.create("").is_none());
    assert!(factory.create("Python").is_none());
}

#[test]
fn test_driver_is_not_parsed_initially() {
    let factory = PythonSyntaxFactory::new();
    let driver = factory.create("python").unwrap();
    assert!(!driver.is_parsed());
}

#[test]
fn test_driver_parse_and_highlights() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("def hello():\n    print(\"hi\")");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..100);
    assert!(!highlights.is_empty(), "Expected highlights for Python code");
}

#[test]
fn test_highlights_contain_keyword() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("def hello():\n    pass");
    let highlights = driver.highlights(0..100);

    let keyword_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("keyword"));

    assert!(keyword_highlight.is_some(), "Expected keyword highlight");
}

#[test]
fn test_highlights_contain_string() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("x = \"hello world\"");
    let highlights = driver.highlights(0..100);

    let string_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("string"));

    assert!(string_highlight.is_some(), "Expected string highlight");
}

#[test]
fn test_highlights_contain_function() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("def hello():\n    pass");
    let highlights = driver.highlights(0..100);

    let fn_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("function"));

    assert!(fn_highlight.is_some(), "Expected function highlight");
}

#[test]
fn test_highlights_contain_comment() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("# this is a comment\nx = 1");
    let highlights = driver.highlights(0..100);

    let comment_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("comment"));

    assert!(comment_highlight.is_some(), "Expected comment highlight");
}

#[test]
fn test_highlights_contain_number() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("x = 42");
    let highlights = driver.highlights(0..100);

    let number_highlight = highlights
        .iter()
        .find(|h| h.category.as_str().starts_with("number"));

    assert!(number_highlight.is_some(), "Expected number highlight");
}

#[test]
fn test_folds() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    let code = "def hello():\n    x = 1\n    y = 2\n    return x + y";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected folds for Python function definition");
}

#[test]
fn test_folds_class() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    let code = "class Foo:\n    def bar(self):\n        pass\n    def baz(self):\n        pass";
    driver.parse(code);
    let folds = driver.folds();

    assert!(!folds.is_empty(), "Expected folds for class definition");
}

#[test]
fn test_folds_empty_file() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("");
    let folds = driver.folds();
    assert!(folds.is_empty(), "Empty file should have no folds");
}

#[test]
fn test_empty_file() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("");
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..0);
    assert!(highlights.is_empty());
}

#[test]
fn test_module_metadata() {
    let module = TreesitterPythonModule::new();
    assert_eq!(module.id(), ModuleId::new("treesitter-python"));
    assert_eq!(module.name(), "Treesitter Python");
    assert_eq!(module.version(), Version::new(0, 10, 0));
}

#[test]
fn test_module_default() {
    fn takes_default<T: Default>(val: T) -> T {
        drop(val);
        T::default()
    }
    let module = takes_default(TreesitterPythonModule::new());
    assert_eq!(module.id(), ModuleId::new("treesitter-python"));
}

#[test]
fn test_module_exit() {
    let mut module = TreesitterPythonModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_factory_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PythonSyntaxFactory>();
}

#[test]
fn test_driver_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
    let factory = PythonSyntaxFactory::new();
    let driver = factory.create("python").unwrap();
    assert_send_sync(&*driver);
}

#[test]
fn test_multiple_factory_instances() {
    let factory1 = PythonSyntaxFactory::new();
    let factory2 = PythonSyntaxFactory::new();

    let mut driver1 = factory1.create("python").unwrap();
    let mut driver2 = factory2.create("python").unwrap();

    driver1.parse("def foo():\n    pass");
    driver2.parse("x = 1");

    assert!(driver1.is_parsed());
    assert!(driver2.is_parsed());

    let h1 = driver1.highlights(0..100);
    let h2 = driver2.highlights(0..100);

    assert!(!h1.is_empty());
    assert!(!h2.is_empty());
}

#[test]
fn test_no_injections() {
    let factory = PythonSyntaxFactory::new();
    let mut driver = factory.create("python").unwrap();

    driver.parse("def hello():\n    pass");
    let injections = driver.injections();
    assert!(injections.is_empty());
}
