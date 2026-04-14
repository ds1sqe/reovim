use reovim_driver_text_syntax::SyntaxDriverFactory;

use super::*;

#[test]
fn test_factory_creation() {
    let factory = JsonSyntaxFactory::new();
    assert!(factory.supports("json"));
    assert!(!factory.supports("rust"));
    assert_eq!(factory.supported_languages(), vec!["json"]);
}

#[test]
fn test_factory_default() {
    let factory = JsonSyntaxFactory::default();
    assert!(factory.supports("json"));
}

#[test]
fn test_create_driver() {
    let factory = JsonSyntaxFactory::new();
    let driver = factory.create("json");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "json");
}

#[test]
fn test_factory_rejects_unknown_language() {
    let factory = JsonSyntaxFactory::new();
    assert!(factory.create("rust").is_none());
    assert!(factory.create("toml").is_none());
}

#[test]
fn test_driver_is_not_parsed_initially() {
    let factory = JsonSyntaxFactory::new();
    let driver = factory.create("json").unwrap();
    assert!(!driver.is_parsed());
}

#[test]
fn test_driver_parse_and_highlights() {
    let factory = JsonSyntaxFactory::new();
    let mut driver = factory.create("json").unwrap();

    let content = r#"{"name": "test", "version": 42, "active": true}"#;
    driver.parse(content);
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..content.len());
    assert!(!highlights.is_empty(), "Expected highlights for JSON content");
}

#[test]
fn test_highlights_contain_string() {
    let factory = JsonSyntaxFactory::new();
    let mut driver = factory.create("json").unwrap();

    driver.parse(r#"{"key": "value"}"#);
    let highlights = driver.highlights(0..100);

    let has_string = highlights.iter().any(|h| h.category.as_str() == "string");
    assert!(has_string, "Expected string highlight, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_property() {
    let factory = JsonSyntaxFactory::new();
    let mut driver = factory.create("json").unwrap();

    driver.parse(r#"{"key": "value"}"#);
    let highlights = driver.highlights(0..100);

    let has_property = highlights.iter().any(|h| h.category.as_str() == "property");
    assert!(has_property, "Expected property highlight, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_number() {
    let factory = JsonSyntaxFactory::new();
    let mut driver = factory.create("json").unwrap();

    driver.parse(r#"{"count": 42}"#);
    let highlights = driver.highlights(0..100);

    let has_number = highlights.iter().any(|h| h.category.as_str() == "number");
    assert!(has_number, "Expected number highlight, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_boolean() {
    let factory = JsonSyntaxFactory::new();
    let mut driver = factory.create("json").unwrap();

    driver.parse(r#"{"active": true}"#);
    let highlights = driver.highlights(0..100);

    let has_bool = highlights.iter().any(|h| h.category.as_str() == "boolean");
    assert!(has_bool, "Expected boolean highlight, got: {highlights:?}");
}

#[test]
fn test_no_folds() {
    let factory = JsonSyntaxFactory::new();
    let mut driver = factory.create("json").unwrap();

    driver.parse(r#"{"key": "value"}"#);
    let folds = driver.folds();
    assert!(folds.is_empty(), "JSON module should not have fold queries");
}

#[test]
fn test_no_injections() {
    let factory = JsonSyntaxFactory::new();
    let mut driver = factory.create("json").unwrap();

    driver.parse(r#"{"key": "value"}"#);
    let injections = driver.injections();
    assert!(injections.is_empty());
}

#[test]
fn test_module_metadata() {
    let module = TreesitterJsonModule::new();
    assert_eq!(module.id(), ModuleId::new("treesitter-json"));
    assert_eq!(module.name(), "Treesitter JSON");
    assert_eq!(module.version(), Version::new(0, 10, 0));
}

#[test]
fn test_module_default() {
    let module = TreesitterJsonModule;
    assert_eq!(module.id(), ModuleId::new("treesitter-json"));
}

#[test]
fn test_module_exit() {
    let mut module = TreesitterJsonModule::new();
    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_init() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let mut module = TreesitterJsonModule::new();
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
    let factory = syntax_store.find("json");
    assert!(factory.is_some(), "JSON factory should be available after module init");
}
