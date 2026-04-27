use super::*;

#[test]
fn test_indent_config_new() {
    let config = IndentConfig::new("rust");
    assert_eq!(config.language_id(), "rust");
    assert_eq!(config.tab_size(), None);
    assert_eq!(config.use_tabs(), None);
}

#[test]
fn test_indent_config_with_tab_size() {
    let config = IndentConfig::new("rust").with_tab_size(4);
    assert_eq!(config.tab_size(), Some(4));
}

#[test]
fn test_indent_config_with_use_tabs() {
    let config = IndentConfig::new("go").with_use_tabs(true);
    assert_eq!(config.use_tabs(), Some(true));
}

#[test]
fn test_indent_config_builder_chain() {
    let config = IndentConfig::new("python")
        .with_tab_size(4)
        .with_use_tabs(false);
    assert_eq!(config.language_id(), "python");
    assert_eq!(config.tab_size(), Some(4));
    assert_eq!(config.use_tabs(), Some(false));
}

#[test]
fn test_indent_config_effective_tab_size_set() {
    let config = IndentConfig::new("rust").with_tab_size(4);
    assert_eq!(config.effective_tab_size(8), 4);
}

#[test]
fn test_indent_config_effective_tab_size_unset() {
    let config = IndentConfig::new("unknown");
    assert_eq!(config.effective_tab_size(8), 8);
}

#[test]
fn test_indent_config_clone() {
    let config = IndentConfig::new("rust").with_tab_size(4);
    #[allow(clippy::redundant_clone)]
    let cloned = config.clone();
    assert_eq!(cloned.language_id(), "rust");
    assert_eq!(cloned.tab_size(), Some(4));
}

#[test]
fn test_indent_config_debug() {
    let config = IndentConfig::new("rust").with_tab_size(4);
    let debug = format!("{config:?}");
    assert!(debug.contains("rust"));
}

#[test]
fn test_default_indent_config() {
    let config = default_indent_config();
    assert_eq!(config.language_id(), "*");
    assert_eq!(config.tab_size(), Some(4));
    assert_eq!(config.use_tabs(), Some(false));
}

#[test]
fn test_store_new_empty() {
    let store = IndentConfigStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_add_and_find() {
    let store = IndentConfigStore::new();
    store.add(IndentConfig::new("rust").with_tab_size(4));
    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());

    let config = store.find("rust").expect("should find rust");
    assert_eq!(config.tab_size(), Some(4));
}

#[test]
fn test_store_find_missing() {
    let store = IndentConfigStore::new();
    assert!(store.find("rust").is_none());
}

#[test]
fn test_store_find_or_default() {
    let store = IndentConfigStore::new();
    store.add(IndentConfig::new("rust").with_tab_size(4));

    let found = store.find_or_default("rust");
    assert_eq!(found.tab_size(), Some(4));

    let fallback = store.find_or_default("unknown");
    assert_eq!(fallback.language_id(), "*");
    assert_eq!(fallback.tab_size(), Some(4));
}

#[test]
fn test_store_multiple_languages() {
    let store = IndentConfigStore::new();
    store.add(
        IndentConfig::new("rust")
            .with_tab_size(4)
            .with_use_tabs(false),
    );
    store.add(IndentConfig::new("go").with_tab_size(8).with_use_tabs(true));
    store.add(
        IndentConfig::new("python")
            .with_tab_size(4)
            .with_use_tabs(false),
    );
    assert_eq!(store.len(), 3);

    let go = store.find("go").expect("should find go");
    assert_eq!(go.tab_size(), Some(8));
    assert_eq!(go.use_tabs(), Some(true));
}

#[test]
fn test_store_debug() {
    let store = IndentConfigStore::new();
    store.add(IndentConfig::new("rust"));
    let debug = format!("{store:?}");
    assert!(debug.contains("IndentConfigStore"));
    assert!(debug.contains('1'));
}

#[test]
fn test_store_default() {
    let store = IndentConfigStore::default();
    assert!(store.is_empty());
}
