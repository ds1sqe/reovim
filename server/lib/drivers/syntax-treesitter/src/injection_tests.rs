use super::*;

#[test]
fn test_injection_manager_default() {
    let manager = InjectionManager::default();
    assert_eq!(manager.layer_count(), 0);
}

#[test]
fn test_injection_manager_new() {
    let manager = InjectionManager::new();
    assert_eq!(manager.layer_count(), 0);
    assert!(!format!("{manager:?}").contains("has_store: true"));
}

#[test]
fn test_injection_manager_with_store() {
    let store = Arc::new(InjectionLayerStore::new());
    let manager = InjectionManager::with_store(store);
    assert_eq!(manager.layer_count(), 0);
    assert!(format!("{manager:?}").contains("has_store: true"));
}

#[test]
fn test_injection_manager_set_store() {
    let mut manager = InjectionManager::new();
    assert!(!format!("{manager:?}").contains("has_store: true"));

    let store = Arc::new(InjectionLayerStore::new());
    manager.set_store(store);
    assert!(format!("{manager:?}").contains("has_store: true"));
}

#[test]
fn test_injection_manager_has_layer() {
    let manager = InjectionManager::new();
    assert!(!manager.has_layer("rust"));
}

#[test]
fn test_injection_manager_debug() {
    let manager = InjectionManager::new();
    let debug = format!("{manager:?}");
    assert!(debug.contains("InjectionManager"));
    assert!(debug.contains("layer_count"));
    assert!(debug.contains("has_store"));
}

#[test]
fn test_injection_manager_invalidate() {
    let mut manager = InjectionManager::new();

    // Invalidate should not panic even with no layers
    manager.invalidate();
    assert_eq!(manager.layer_count(), 0);
}

#[test]
fn test_injection_manager_highlight_injections_empty() {
    let mut manager = InjectionManager::new();

    let injections: Vec<Injection> = vec![];
    let highlights = manager.highlight_injections(&injections, "content", 0..100);

    assert!(highlights.is_empty());
}

#[test]
fn test_injection_manager_highlight_injections_no_layer() {
    let mut manager = InjectionManager::new();

    // Injection for a language we don't have a layer for
    let injections = vec![Injection::new("rust".to_string(), 10..50, 0, 0, 2, 10)];

    let highlights = manager.highlight_injections(&injections, "fn main() {}", 0..100);

    // Should return empty since no rust layer is registered
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_layer_creation() {
    // Test creating an injection layer with Rust
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();

    // Create a minimal highlights query
    let query = Query::new(&language, "(identifier) @variable").unwrap();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query));

    assert!(layer.is_some());
    let layer = layer.unwrap();
    assert_eq!(layer.language_id(), "rust");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_injection_layer_highlight() {
    // Test highlighting embedded Rust code
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();

    // Create a query that matches identifiers
    let query = Query::new(&language, "(identifier) @variable").unwrap();

    let mut layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();

    // Simulate embedded Rust code in a larger document
    // Full content: "Some text\n```rust\nlet x = 1;\n```\nMore text"
    // Rust portion at bytes 17..28: "let x = 1;"
    let full_content = "Some text\n```rust\nlet x = 1;\n```\nMore text";

    let injection = Injection::new(
        "rust".to_string(),
        18..28, // "let x = 1;"
        2,      // start_row
        0,      // start_col
        2,      // end_row
        10,     // end_col
    );

    let highlights = layer.highlight_injection(&injection, full_content);

    // Should have at least one highlight for 'x'
    assert!(!highlights.is_empty(), "Expected highlights for identifier 'x'");

    // Check that the highlights are offset-adjusted
    for h in &highlights {
        assert!(h.start_byte >= 18, "Highlight start should be offset to parent coordinates");
        assert!(h.end_byte <= 28, "Highlight end should be within injection range");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_injection_manager_with_layer() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();

    let mut manager = InjectionManager::new();

    // Register a Rust layer
    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    assert!(manager.has_layer("rust"));
    assert_eq!(manager.layer_count(), 1);

    // Test highlighting with the registered layer
    let full_content = "Some text\n```rust\nlet x = 1;\n```\nMore text";
    let injections = vec![Injection::new(
        "rust".to_string(),
        18..28, // "let x = 1;"
        2,
        0,
        2,
        10,
    )];

    let highlights = manager.highlight_injections(&injections, full_content, 0..100);

    // Should have highlights now
    assert!(!highlights.is_empty(), "Expected highlights with registered layer");
}

// ========================================================================
// InjectionLayerStore Tests
// ========================================================================

/// Mock factory for testing the store (always returns None).
struct MockLayerFactory {
    language: &'static str,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl InjectionLayerFactory for MockLayerFactory {
    fn create_layer(&self) -> Option<InjectionLayer> {
        // For testing, we don't actually create a layer
        None
    }

    fn language_id(&self) -> &'static str {
        self.language
    }
}

/// Real factory that creates a working Rust injection layer.
struct RealRustLayerFactory {
    language: tree_sitter::Language,
    query: Arc<Query>,
}

impl RealRustLayerFactory {
    fn new() -> Self {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        Self { language, query }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl InjectionLayerFactory for RealRustLayerFactory {
    fn create_layer(&self) -> Option<InjectionLayer> {
        InjectionLayer::new("rust", &self.language, self.query.clone())
    }

    fn language_id(&self) -> &'static str {
        "rust"
    }
}

#[test]
fn test_injection_layer_store_new_empty() {
    let store = InjectionLayerStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_injection_layer_store_add_and_find() {
    let store = InjectionLayerStore::new();

    store.add(Arc::new(MockLayerFactory { language: "rust" }));
    store.add(Arc::new(MockLayerFactory { language: "python" }));

    assert_eq!(store.len(), 2);
    assert!(store.find("rust").is_some());
    assert!(store.find("python").is_some());
    assert!(store.find("javascript").is_none());
}

#[test]
fn test_injection_layer_store_supported_languages() {
    let store = InjectionLayerStore::new();

    store.add(Arc::new(MockLayerFactory { language: "rust" }));
    store.add(Arc::new(MockLayerFactory { language: "python" }));

    let languages = store.supported_languages();
    assert_eq!(languages.len(), 2);
    assert!(languages.contains(&"rust".to_string()));
    assert!(languages.contains(&"python".to_string()));
}

#[test]
fn test_injection_layer_store_debug() {
    let store = InjectionLayerStore::new();
    store.add(Arc::new(MockLayerFactory { language: "rust" }));

    let debug = format!("{store:?}");
    assert!(debug.contains("InjectionLayerStore"));
    assert!(debug.contains("count"));
}

#[test]
fn test_injection_layer_store_default() {
    let store = InjectionLayerStore::default();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_injection_manager_register_and_has() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    assert!(manager.has_layer("rust"));
    assert!(!manager.has_layer("python"));
    assert_eq!(manager.layer_count(), 1);
}

#[test]
fn test_injection_manager_highlight_out_of_range_skipped() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    // Create an injection outside the query range
    let injections = vec![Injection::new("rust".to_string(), 100..200, 5, 0, 10, 0)];

    // Query range 0..50 does not overlap injection 100..200
    let highlights = manager.highlight_injections(&injections, "fn main() {}", 0..50);
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_layer_clear_cache() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();

    // Parse something to populate cache
    let content = "Some text\n```rust\nlet x = 1;\n```\nMore text";
    let injection = Injection::new("rust".to_string(), 18..28, 2, 0, 2, 10);
    let _highlights = layer.highlight_injection(&injection, content);

    // Now clear cache
    layer.clear_cache();

    // Re-highlight after clear (should re-parse without error)
    let highlights = layer.highlight_injection(&injection, content);
    assert!(!highlights.is_empty());
}

#[test]
fn test_injection_layer_highlight_empty_content() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();

    // Empty injection content
    let injection = Injection::new("rust".to_string(), 5..5, 0, 0, 0, 0);
    let highlights = layer.highlight_injection(&injection, "hello");
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_layer_highlight_out_of_bounds() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();

    // Range beyond content length
    let injection = Injection::new("rust".to_string(), 100..200, 0, 0, 0, 0);
    let highlights = layer.highlight_injection(&injection, "short");
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_manager_get_or_create_layer_no_store() {
    let mut manager = InjectionManager::new();

    // No store configured — always returns None for unknown languages
    let result = manager.get_or_create_layer("unknown");
    assert!(result.is_none());
}

#[test]
fn test_injection_manager_get_or_create_layer_store_factory_returns_none() {
    // MockLayerFactory.create_layer() returns None
    let store = Arc::new(InjectionLayerStore::new());
    store.add(Arc::new(MockLayerFactory { language: "rust" }));

    let mut manager = InjectionManager::with_store(store);

    // Store has a factory for "rust" but it returns None from create_layer()
    let result = manager.get_or_create_layer("rust");
    assert!(result.is_none());
    assert_eq!(manager.layer_count(), 0);
}

#[test]
fn test_injection_manager_get_or_create_layer_cached() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    // Pre-register a layer
    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    // get_or_create_layer should find the cached layer (no store needed)
    let result = manager.get_or_create_layer("rust");
    assert!(result.is_some());
    assert_eq!(result.unwrap().language_id(), "rust");
}

#[test]
fn test_injection_manager_get_or_create_layer_dynamic_creation() {
    let store = Arc::new(InjectionLayerStore::new());
    store.add(Arc::new(RealRustLayerFactory::new()));

    let mut manager = InjectionManager::with_store(store);
    assert_eq!(manager.layer_count(), 0);

    // First call: dynamically creates the layer
    let result = manager.get_or_create_layer("rust");
    assert!(result.is_some());
    assert_eq!(result.unwrap().language_id(), "rust");
    assert_eq!(manager.layer_count(), 1);

    // Second call: returns cached layer
    let result = manager.get_or_create_layer("rust");
    assert!(result.is_some());
    assert_eq!(manager.layer_count(), 1);
}

#[test]
fn test_injection_manager_get_or_create_layer_unknown_language_with_store() {
    let store = Arc::new(InjectionLayerStore::new());
    store.add(Arc::new(MockLayerFactory { language: "rust" }));

    let mut manager = InjectionManager::with_store(store);

    // "python" not in store — returns None
    let result = manager.get_or_create_layer("python");
    assert!(result.is_none());
}

#[test]
fn test_highlight_injections_dynamic_creation() {
    let store = Arc::new(InjectionLayerStore::new());
    store.add(Arc::new(RealRustLayerFactory::new()));

    let mut manager = InjectionManager::with_store(store);

    // Simulate: parent Markdown doc has embedded Rust at bytes 10..30
    let full_content = "# Title\n\nfn main() { let x = 1; }extra";
    let injection = Injection::new("rust".to_string(), 10..34, 1, 0, 1, 24);

    // No layer pre-registered — should be created dynamically
    assert_eq!(manager.layer_count(), 0);

    let highlights =
        manager.highlight_injections(&[injection], full_content, 0..full_content.len());

    // Layer was dynamically created
    assert_eq!(manager.layer_count(), 1);
    assert!(manager.has_layer("rust"));

    // Should have produced highlights from the Rust code
    assert!(
        !highlights.is_empty(),
        "Expected highlights from dynamically created Rust injection layer"
    );
}

#[test]
fn test_highlight_injections_skips_unsupported_language() {
    // Store with only Rust — injection for "python" should be silently skipped
    let store = Arc::new(InjectionLayerStore::new());
    store.add(Arc::new(MockLayerFactory { language: "rust" }));

    let mut manager = InjectionManager::with_store(store);

    let content = "print('hello')";
    let injection = Injection::new("python".to_string(), 0..14, 0, 0, 0, 14);
    let highlights = manager.highlight_injections(&[injection], content, 0..content.len());

    assert!(highlights.is_empty());
    assert_eq!(manager.layer_count(), 0);
}

#[test]
fn test_injection_manager_highlight_non_overlapping_injection() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    // Injection range 10..50, query range 60..100 => no overlap
    let injections = vec![Injection::new("rust".to_string(), 10..50, 0, 0, 2, 0)];
    let highlights = manager.highlight_injections(&injections, "let x = 1;", 60..100);
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_layer_store_service_impl() {
    fn accepts_service(_: &dyn reovim_kernel::api::v1::Service) {}
    let store = InjectionLayerStore::new();
    accepts_service(&store);
}

#[test]
fn test_injection_manager_invalidate_with_layers() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    // Highlight to populate cache
    let content = "Some text\n```rust\nlet x = 1;\n```\nMore text";
    let injections = vec![Injection::new("rust".to_string(), 18..28, 2, 0, 2, 10)];
    let _highlights = manager.highlight_injections(&injections, content, 0..100);

    // Invalidate should not panic and should clear caches
    manager.invalidate();
    assert_eq!(manager.layer_count(), 1); // Layer still registered
}

#[test]
fn test_injection_layer_store_debug_with_languages() {
    let store = InjectionLayerStore::new();
    store.add(Arc::new(MockLayerFactory { language: "rust" }));
    store.add(Arc::new(MockLayerFactory { language: "python" }));

    let debug = format!("{store:?}");
    assert!(debug.contains("InjectionLayerStore"));
    assert!(debug.contains("rust"));
    assert!(debug.contains("python"));
    assert!(debug.contains('2'));
}

#[test]
fn test_injection_layer_store_find_returns_none_for_unknown() {
    let store = InjectionLayerStore::new();
    store.add(Arc::new(MockLayerFactory { language: "rust" }));
    assert!(store.find("javascript").is_none());
}

#[test]
fn test_injection_layer_store_supported_languages_empty() {
    let store = InjectionLayerStore::new();
    let langs = store.supported_languages();
    assert!(langs.is_empty());
}

#[test]
fn test_injection_manager_debug_with_layers() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    let debug = format!("{manager:?}");
    assert!(debug.contains("InjectionManager"));
    assert!(debug.contains("rust"));
    assert!(debug.contains("layer_count"));
}

#[test]
fn test_injection_manager_highlight_injection_end_at_start_of_range() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    // Injection ends exactly where query range starts: no overlap
    let injections = vec![Injection::new("rust".to_string(), 0..10, 0, 0, 0, 10)];
    let highlights = manager.highlight_injections(&injections, "let x = 1;", 10..20);
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_manager_highlight_injection_start_at_end_of_range() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();
    manager.register_layer(layer);

    // Injection starts exactly where query range ends: no overlap
    let injections = vec![Injection::new("rust".to_string(), 20..30, 0, 0, 0, 10)];
    let highlights =
        manager.highlight_injections(&injections, "let x = 1;let y = 2;let z = 3;", 0..20);
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_layer_highlight_reuses_cached_tree() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query = Query::new(&language, "(identifier) @variable").unwrap();
    let mut layer = InjectionLayer::new("rust", &language, Arc::new(query)).unwrap();

    let content = "Some text\n```rust\nlet x = 1;\n```\nMore text";
    let injection = Injection::new("rust".to_string(), 18..28, 2, 0, 2, 10);

    // First call populates cache
    let h1 = layer.highlight_injection(&injection, content);
    assert!(!h1.is_empty());

    // Second call reuses cached tree
    let h2 = layer.highlight_injection(&injection, content);
    assert!(!h2.is_empty());
    assert_eq!(h1.len(), h2.len());
}

#[test]
fn test_injection_manager_register_replaces_existing_layer() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query1 = Query::new(&language, "(identifier) @variable").unwrap();
    let query2 = Query::new(&language, "(identifier) @variable").unwrap();
    let mut manager = InjectionManager::new();

    let layer1 = InjectionLayer::new("rust", &language, Arc::new(query1)).unwrap();
    manager.register_layer(layer1);
    assert_eq!(manager.layer_count(), 1);

    // Re-registering with same language_id replaces
    let layer2 = InjectionLayer::new("rust", &language, Arc::new(query2)).unwrap();
    manager.register_layer(layer2);
    assert_eq!(manager.layer_count(), 1); // Still 1, replaced
}

#[test]
fn test_injection_layer_store_not_empty() {
    let store = InjectionLayerStore::new();
    assert!(store.is_empty());
    store.add(Arc::new(MockLayerFactory { language: "rust" }));
    assert!(!store.is_empty());
    assert_eq!(store.len(), 1);
}

#[test]
fn test_highlight_injections_skips_existing_layer() {
    // Store contains a real factory for "rust"
    let store = Arc::new(InjectionLayerStore::new());
    store.add(Arc::new(RealRustLayerFactory::new()));

    let mut manager = InjectionManager::with_store(store);

    let full_content = "# Title\n\nfn main() { let x = 1; }extra";
    let injection = Injection::new("rust".to_string(), 10..34, 1, 0, 1, 24);

    // First call: dynamically creates the layer
    let _ = manager.highlight_injections(
        std::slice::from_ref(&injection),
        full_content,
        0..full_content.len(),
    );
    assert_eq!(manager.layer_count(), 1);

    // Second call with same injection: layer already exists in self.layers,
    // so `!self.layers.contains_key(...)` is false (line 295 false branch)
    let highlights = manager.highlight_injections(
        std::slice::from_ref(&injection),
        full_content,
        0..full_content.len(),
    );

    // Layer count unchanged — no duplicate creation
    assert_eq!(manager.layer_count(), 1);
    assert!(!highlights.is_empty());
}

#[test]
fn test_highlight_injections_skips_when_create_layer_returns_none() {
    // MockLayerFactory's create_layer() returns None
    let store = Arc::new(InjectionLayerStore::new());
    store.add(Arc::new(MockLayerFactory { language: "rust" }));

    let mut manager = InjectionManager::with_store(store);

    let content = "fn main() {}";
    let injection = Injection::new("rust".to_string(), 0..12, 0, 0, 0, 12);

    // Store finds factory for "rust" but create_layer() returns None
    // (line 297 false branch)
    let highlights = manager.highlight_injections(&[injection], content, 0..content.len());

    assert!(highlights.is_empty());
    assert_eq!(manager.layer_count(), 0);
}
