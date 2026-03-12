use super::*;

// =========================================================================
// SnippetDefinition
// =========================================================================

#[test]
fn test_snippet_definition_clone() {
    let def = SnippetDefinition {
        name: "test".to_string(),
        prefix: "t".to_string(),
        body_raw: "$1".to_string(),
        description: Some("A test snippet".to_string()),
        scope: None,
    };
    let cloned = def.clone();
    assert_eq!(def, cloned);
}

#[test]
fn test_snippet_definition_debug() {
    let def = SnippetDefinition {
        name: "test".to_string(),
        prefix: "t".to_string(),
        body_raw: "$1".to_string(),
        description: None,
        scope: None,
    };
    let debug = format!("{def:?}");
    assert!(debug.contains("SnippetDefinition"));
}

#[test]
fn test_snippet_definition_no_description() {
    let def = SnippetDefinition {
        name: "bare".to_string(),
        prefix: "b".to_string(),
        body_raw: "body".to_string(),
        description: None,
        scope: None,
    };
    assert!(def.description.is_none());
}

// =========================================================================
// Mock provider for testing
// =========================================================================

struct MockProvider {
    snippets: Vec<(String, SnippetDefinition)>, // (filetype, def)
}

impl MockProvider {
    fn new(snippets: Vec<(String, SnippetDefinition)>) -> Self {
        Self { snippets }
    }
}

impl SnippetProvider for MockProvider {
    fn snippets_for_filetype(&self, filetype: &str) -> Vec<&SnippetDefinition> {
        self.snippets
            .iter()
            .filter(|(ft, _)| ft == filetype)
            .map(|(_, def)| def)
            .collect()
    }

    fn snippet_by_prefix(&self, filetype: &str, prefix: &str) -> Option<&SnippetDefinition> {
        self.snippets
            .iter()
            .find(|(ft, def)| ft == filetype && def.prefix == prefix)
            .map(|(_, def)| def)
    }
}

fn make_def(name: &str, prefix: &str, body: &str) -> SnippetDefinition {
    SnippetDefinition {
        name: name.to_string(),
        prefix: prefix.to_string(),
        body_raw: body.to_string(),
        description: None,
        scope: None,
    }
}

// =========================================================================
// SnippetRegistry
// =========================================================================

#[test]
fn test_registry_new_empty() {
    let registry = SnippetRegistry::new();
    assert_eq!(registry.provider_count(), 0);
}

#[test]
fn test_registry_default() {
    let registry = SnippetRegistry::default();
    assert_eq!(registry.provider_count(), 0);
}

#[test]
fn test_registry_register() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![])));
    assert_eq!(registry.provider_count(), 1);
}

#[test]
fn test_registry_find_by_prefix() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("function", "fn", "fn $1() {}"),
    )])));

    let found = registry.find_by_prefix("rust", "fn");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "function");
}

#[test]
fn test_registry_find_by_prefix_not_found() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("function", "fn", "fn $1() {}"),
    )])));

    assert!(registry.find_by_prefix("rust", "xyz").is_none());
}

#[test]
fn test_registry_find_by_prefix_wrong_filetype() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("function", "fn", "fn $1() {}"),
    )])));

    assert!(registry.find_by_prefix("python", "fn").is_none());
}

#[test]
fn test_registry_find_by_prefix_first_provider_wins() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("first", "fn", "first body"),
    )])));
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("second", "fn", "second body"),
    )])));

    let found = registry.find_by_prefix("rust", "fn").unwrap();
    assert_eq!(found.name, "first");
}

#[test]
fn test_registry_all_for_filetype() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![
        ("rust".to_string(), make_def("function", "fn", "fn $1()")),
        ("rust".to_string(), make_def("struct", "st", "struct $1 {}")),
        ("python".to_string(), make_def("def", "def", "def $1(): pass")),
    ])));

    let rust_snippets = registry.all_for_filetype("rust");
    assert_eq!(rust_snippets.len(), 2);
}

#[test]
fn test_registry_all_for_filetype_aggregates_providers() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("fn", "fn", "fn $1()"),
    )])));
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("struct", "st", "struct $1 {}"),
    )])));

    let all = registry.all_for_filetype("rust");
    assert_eq!(all.len(), 2);
}

#[test]
fn test_registry_all_for_filetype_empty() {
    let registry = SnippetRegistry::new();
    assert!(registry.all_for_filetype("rust").is_empty());
}

#[test]
fn test_registry_find_empty() {
    let registry = SnippetRegistry::new();
    assert!(registry.find_by_prefix("rust", "fn").is_none());
}

// =========================================================================
// Global fallback (#529)
// =========================================================================

#[test]
fn test_global_fallback_when_filetype_has_no_match() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "global".to_string(),
        make_def("test_expand", "tst", "TEST"),
    )])));

    // Searching "rust" should fall back to "global"
    let found = registry.find_by_prefix("rust", "tst");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "test_expand");
}

#[test]
fn test_filetype_specific_takes_precedence_over_global() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![
        ("rust".to_string(), make_def("rust_fn", "fn", "fn rust")),
        ("global".to_string(), make_def("global_fn", "fn", "fn global")),
    ])));

    let found = registry.find_by_prefix("rust", "fn").unwrap();
    assert_eq!(found.name, "rust_fn");
}

#[test]
fn test_global_not_duplicated_when_filetype_is_global() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "global".to_string(),
        make_def("test", "tst", "TEST"),
    )])));

    // When filetype is already "global", should not search twice
    let all = registry.all_for_filetype("global");
    assert_eq!(all.len(), 1);
}

#[test]
fn test_all_for_filetype_includes_global() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![
        ("rust".to_string(), make_def("rust_fn", "fn", "fn $1()")),
        ("global".to_string(), make_def("global_tst", "tst", "TEST")),
    ])));

    let all = registry.all_for_filetype("rust");
    assert_eq!(all.len(), 2);
}

// =========================================================================
// applies_to / scope (#529)
// =========================================================================

#[test]
fn test_scope_none_applies_to_all() {
    let def = make_def("test", "t", "$1");
    assert!(def.applies_to("rust"));
    assert!(def.applies_to("python"));
    assert!(def.applies_to("global"));
}

#[test]
fn test_scope_applies_to_matching_filetype() {
    let def = SnippetDefinition {
        scope: Some(vec!["rust".to_string(), "toml".to_string()]),
        ..make_def("test", "t", "$1")
    };
    assert!(def.applies_to("rust"));
    assert!(def.applies_to("toml"));
}

#[test]
fn test_scope_blocks_non_matching_filetype() {
    let def = SnippetDefinition {
        scope: Some(vec!["rust".to_string()]),
        ..make_def("test", "t", "$1")
    };
    assert!(!def.applies_to("python"));
    assert!(!def.applies_to("global"));
}

#[test]
fn test_scope_empty_vec_applies_to_none() {
    // Empty scope vec (shouldn't normally occur, but defensive)
    let def = SnippetDefinition {
        scope: Some(vec![]),
        ..make_def("test", "t", "$1")
    };
    assert!(!def.applies_to("rust"));
}

#[test]
fn test_scope_filtering_in_find_by_prefix() {
    let mut registry = SnippetRegistry::new();
    // Snippet scoped to javascript only, stored under "global"
    let scoped_def = SnippetDefinition {
        scope: Some(vec!["javascript".to_string()]),
        ..make_def("js_only", "log", "console.log($1)")
    };
    registry.register(Box::new(MockProvider::new(vec![("global".to_string(), scoped_def)])));

    // Should find it when searching as javascript
    assert!(registry.find_by_prefix("javascript", "log").is_some());
    // Should NOT find it when searching as rust
    assert!(registry.find_by_prefix("rust", "log").is_none());
}

// =========================================================================
// Filetype inheritance (#529)
// =========================================================================

#[test]
fn test_typescript_inherits_javascript_snippets() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "javascript".to_string(),
        make_def("console_log", "log", "console.log($1)"),
    )])));

    // TypeScript should find JavaScript snippet via inheritance
    let found = registry.find_by_prefix("typescript", "log");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "console_log");
}

#[test]
fn test_typescriptreact_inherits_ts_and_js() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![
        ("typescript".to_string(), make_def("ts_only", "tso", "TS_ONLY")),
        ("javascript".to_string(), make_def("js_only", "jso", "JS_ONLY")),
    ])));

    // typescriptreact should find both ts and js snippets
    assert!(registry.find_by_prefix("typescriptreact", "tso").is_some());
    assert!(registry.find_by_prefix("typescriptreact", "jso").is_some());
}

#[test]
fn test_filetype_specific_takes_precedence_over_inherited() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![
        ("typescript".to_string(), make_def("ts_fn", "fn", "TS function")),
        ("javascript".to_string(), make_def("js_fn", "fn", "JS function")),
    ])));

    // TypeScript-specific should win over inherited JavaScript
    let found = registry.find_by_prefix("typescript", "fn").unwrap();
    assert_eq!(found.name, "ts_fn");
}

#[test]
fn test_all_for_filetype_includes_inherited() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![
        ("typescript".to_string(), make_def("ts_fn", "fn", "TS")),
        ("javascript".to_string(), make_def("js_log", "log", "JS")),
        ("global".to_string(), make_def("global_tst", "tst", "G")),
    ])));

    let all = registry.all_for_filetype("typescript");
    assert_eq!(all.len(), 3); // ts + js + global
}

// =========================================================================
// SnippetRegistryHandle (#529)
// =========================================================================

#[test]
fn test_handle_find_by_prefix() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("function", "fn", "fn $1() {}"),
    )])));
    let handle = SnippetRegistryHandle::new(registry);

    let found = handle.find_by_prefix("rust", "fn");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "function");
}

#[test]
fn test_handle_find_by_prefix_not_found() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    assert!(handle.find_by_prefix("rust", "xyz").is_none());
}

#[test]
fn test_handle_all_for_filetype() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![
        ("rust".to_string(), make_def("fn", "fn", "fn $1()")),
        ("rust".to_string(), make_def("st", "st", "struct $1 {}")),
    ])));
    let handle = SnippetRegistryHandle::new(registry);

    let all = handle.all_for_filetype("rust");
    assert_eq!(all.len(), 2);
}

#[test]
fn test_handle_all_for_filetype_empty() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    assert!(handle.all_for_filetype("rust").is_empty());
}

#[test]
fn test_handle_replace_swaps_registry() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());

    // Initially empty
    assert!(handle.find_by_prefix("rust", "fn").is_none());

    // Replace with a registry that has a snippet
    let mut new_registry = SnippetRegistry::new();
    new_registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("function", "fn", "fn $1() {}"),
    )])));
    handle.replace(new_registry);

    // Now it should find the snippet
    assert!(handle.find_by_prefix("rust", "fn").is_some());
}

#[test]
fn test_handle_clone_shares_registry() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "rust".to_string(),
        make_def("function", "fn", "fn $1() {}"),
    )])));
    let handle1 = SnippetRegistryHandle::new(registry);
    let handle2 = handle1.clone();

    // Both should find the same snippet
    assert!(handle1.find_by_prefix("rust", "fn").is_some());
    assert!(handle2.find_by_prefix("rust", "fn").is_some());

    // Replace via one handle
    handle1.replace(SnippetRegistry::new());

    // Both should see the empty registry now
    assert!(handle2.find_by_prefix("rust", "fn").is_none());
}

#[test]
fn test_handle_poisoned_lock_replace() {
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    let handle2 = handle.clone();

    // Poison the lock by panicking while holding write guard
    let _ = std::thread::spawn(move || {
        let _guard = handle2.0.write().unwrap();
        panic!("intentional poison");
    })
    .join();

    // replace should silently handle the poisoned lock
    handle.replace(SnippetRegistry::new());

    // find_by_prefix should return None on poisoned lock
    assert!(handle.find_by_prefix("rust", "fn").is_none());

    // all_for_filetype should return empty vec on poisoned lock
    assert!(handle.all_for_filetype("rust").is_empty());
}

#[test]
fn test_scss_inherits_css() {
    let mut registry = SnippetRegistry::new();
    registry.register(Box::new(MockProvider::new(vec![(
        "css".to_string(),
        make_def("display", "dis", "display: $1;"),
    )])));

    assert!(registry.find_by_prefix("scss", "dis").is_some());
    assert!(registry.find_by_prefix("sass", "dis").is_some());
    assert!(registry.find_by_prefix("less", "dis").is_some());
}
