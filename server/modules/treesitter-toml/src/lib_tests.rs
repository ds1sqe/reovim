use reovim_driver_syntax::SyntaxDriverFactory;

use super::*;

#[test]
fn test_factory_creation() {
    let factory = TomlSyntaxFactory::new();
    assert!(factory.supports("toml"));
    assert!(!factory.supports("rust"));
    assert_eq!(factory.supported_languages(), vec!["toml"]);
}

#[test]
fn test_factory_default() {
    let factory = TomlSyntaxFactory::default();
    assert!(factory.supports("toml"));
}

#[test]
fn test_create_driver() {
    let factory = TomlSyntaxFactory::new();
    let driver = factory.create("toml");
    assert!(driver.is_some());
    assert_eq!(driver.unwrap().language(), "toml");
}

#[test]
fn test_factory_rejects_unknown_language() {
    let factory = TomlSyntaxFactory::new();
    assert!(factory.create("rust").is_none());
    assert!(factory.create("json").is_none());
}

#[test]
fn test_driver_is_not_parsed_initially() {
    let factory = TomlSyntaxFactory::new();
    let driver = factory.create("toml").unwrap();
    assert!(!driver.is_parsed());
}

#[test]
fn test_driver_parse_and_highlights() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    let content = "[package]\nname = \"test\"\nversion = \"0.1.0\"";
    driver.parse(content);
    assert!(driver.is_parsed());

    let highlights = driver.highlights(0..content.len());
    assert!(!highlights.is_empty(), "Expected highlights for TOML content");
}

#[test]
fn test_highlights_contain_string() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("name = \"test\"");
    let highlights = driver.highlights(0..100);

    let has_string = highlights.iter().any(|h| h.category.as_str() == "string");
    assert!(has_string, "Expected string highlight, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_property() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("name = \"test\"");
    let highlights = driver.highlights(0..100);

    let has_property = highlights.iter().any(|h| h.category.as_str() == "property");
    assert!(has_property, "Expected property highlight, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_type_for_table() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("[package]\nname = \"test\"");
    let highlights = driver.highlights(0..100);

    let has_type = highlights.iter().any(|h| h.category.as_str() == "type");
    assert!(has_type, "Expected type highlight for table header, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_number() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("port = 8080");
    let highlights = driver.highlights(0..100);

    let has_number = highlights.iter().any(|h| h.category.as_str() == "number");
    assert!(has_number, "Expected number highlight, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_boolean() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("enabled = true");
    let highlights = driver.highlights(0..100);

    let has_bool = highlights.iter().any(|h| h.category.as_str() == "boolean");
    assert!(has_bool, "Expected boolean highlight, got: {highlights:?}");
}

#[test]
fn test_highlights_contain_comment() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("# This is a comment\nname = \"test\"");
    let highlights = driver.highlights(0..100);

    let has_comment = highlights.iter().any(|h| h.category.as_str() == "comment");
    assert!(has_comment, "Expected comment highlight, got: {highlights:?}");
}

#[test]
fn test_no_folds() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("[package]\nname = \"test\"");
    let folds = driver.folds();
    assert!(folds.is_empty(), "TOML module should not have fold queries");
}

#[test]
fn test_no_injections() {
    let factory = TomlSyntaxFactory::new();
    let mut driver = factory.create("toml").unwrap();

    driver.parse("[package]\nname = \"test\"");
    let injections = driver.injections();
    assert!(injections.is_empty());
}

#[test]
fn test_module_metadata() {
    let module = TreesitterTomlModule::new();
    assert_eq!(module.id(), ModuleId::new("treesitter-toml"));
    assert_eq!(module.name(), "Treesitter TOML");
    assert_eq!(module.version(), Version::new(0, 10, 0));
}

#[test]
fn test_module_default() {
    let module = TreesitterTomlModule;
    assert_eq!(module.id(), ModuleId::new("treesitter-toml"));
}

#[test]
fn test_module_exit() {
    let mut module = TreesitterTomlModule::new();
    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_init() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let mut module = TreesitterTomlModule::new();
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
    let factory = syntax_store.find("toml");
    assert!(factory.is_some(), "TOML factory should be available after module init");
}
