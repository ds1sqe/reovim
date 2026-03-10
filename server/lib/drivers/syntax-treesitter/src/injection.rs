//! Injection layer management for embedded language highlighting.
//!
//! This module provides `InjectionLayer` and `InjectionManager` for parsing
//! and highlighting embedded languages (e.g., code blocks in Markdown).
//!
//! # Architecture
//!
//! ```text
//! TreeSitterDriver (parent - e.g., Markdown)
//!   └─ InjectionManager
//!        └─ InjectionLayer (child - e.g., Rust)
//!             └─ Parser + Tree for the embedded content
//! ```
//!
//! # Usage
//!
//! The `InjectionManager` is optionally created by `TreeSitterDriver::with_queries()`
//! when an injections query is provided. It:
//!
//! 1. Detects injection regions via the parent's `injections()` method
//! 2. Creates child parsers/layers for detected languages (lazily)
//! 3. Parses embedded content and produces highlights
//! 4. Merges child highlights with parent highlights (offset-adjusted)

use std::{collections::HashMap, ops::Range, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_driver_syntax::{Annotation, HighlightCategory, Injection},
    tree_sitter::{Parser, Query, Tree},
};

/// An injection layer for parsing and highlighting embedded content.
///
/// Each layer has its own tree-sitter parser and cached tree for a specific
/// language embedded within the parent document.
pub struct InjectionLayer {
    /// Language identifier (e.g., "rust", "python")
    language_id: String,
    /// Tree-sitter parser for this language
    parser: Mutex<Parser>,
    /// Cached parse tree (keyed by byte range)
    trees: HashMap<(usize, usize), Tree>,
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
}

impl InjectionLayer {
    /// Create a new injection layer for a language.
    ///
    /// Returns `None` if the parser can't be configured for the language.
    #[must_use]
    pub fn new(
        language_id: &str,
        ts_language: &tree_sitter::Language,
        highlight_query: Arc<Query>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(ts_language).ok()?;

        Some(Self {
            language_id: language_id.to_string(),
            parser: Mutex::new(parser),
            trees: HashMap::new(),
            highlight_query,
        })
    }

    /// Get the language ID.
    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    /// Parse embedded content and return highlights.
    ///
    /// The highlights are offset-adjusted to match positions in the parent document.
    #[allow(clippy::cast_possible_truncation)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn highlight_injection(
        &mut self,
        injection: &Injection,
        full_content: &str,
    ) -> Vec<Annotation> {
        // Extract the embedded content
        let range = injection.byte_range.clone();
        if range.start >= full_content.len() || range.end > full_content.len() {
            return Vec::new();
        }

        let embedded_content = &full_content[range.clone()];
        if embedded_content.is_empty() {
            return Vec::new();
        }

        // Get or create cached tree
        let cache_key = (range.start, range.end);
        if !self.trees.contains_key(&cache_key) {
            let mut parser = self.parser.lock();
            if let Some(tree) = parser.parse(embedded_content, None) {
                self.trees.insert(cache_key, tree);
            } else {
                return Vec::new();
            }
        }

        let Some(tree) = self.trees.get(&cache_key) else {
            return Vec::new();
        };

        // Query highlights
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut highlights = Vec::new();
        let capture_names = self.highlight_query.capture_names();

        let mut matches =
            cursor.matches(&self.highlight_query, tree.root_node(), embedded_content.as_bytes());

        while let Some(match_) = streaming_iterator::StreamingIterator::next(&mut matches) {
            for capture in match_.captures {
                let capture_name = &capture_names[capture.index as usize];

                // Skip non-highlight captures
                if capture_name.starts_with("decoration.")
                    || capture_name.starts_with("textobject.")
                    || capture_name.starts_with("local.")
                    || capture_name.contains(".inner")
                    || capture_name.contains(".outer")
                {
                    continue;
                }

                let node = capture.node;

                // Offset byte positions to parent document coordinates
                let start_byte = range.start + node.start_byte();
                let end_byte = range.start + node.end_byte();

                highlights.push(Annotation::highlight(
                    start_byte,
                    end_byte,
                    HighlightCategory::new(*capture_name),
                ));
            }
        }

        highlights
    }

    /// Clear the parse tree cache.
    pub fn clear_cache(&mut self) {
        self.trees.clear();
    }
}

/// Factory trait for creating injection layers.
///
/// Language modules implement this trait to enable their language to be used
/// as an embedded language within other documents (e.g., Rust code blocks
/// inside Markdown files).
///
/// This trait lives in the driver crate (not the traits crate) because it
/// requires tree-sitter types that should not pollute the trait boundary.
///
/// # Example
///
/// ```ignore
/// impl InjectionLayerFactory for RustSyntaxFactory {
///     fn create_layer(&self) -> Option<InjectionLayer> {
///         let language = tree_sitter_rust::LANGUAGE;
///         InjectionLayer::new("rust", &language.into(), self.highlight_query.clone())
///     }
///
///     fn language_id(&self) -> &'static str {
///         "rust"
///     }
/// }
/// ```
pub trait InjectionLayerFactory: Send + Sync {
    /// Create an injection layer for embedded language highlighting.
    ///
    /// Returns `None` if the layer cannot be created (e.g., query compilation failure).
    fn create_layer(&self) -> Option<InjectionLayer>;

    /// The language ID this factory supports (e.g., "rust", "python").
    fn language_id(&self) -> &'static str;
}

/// Manages injection detection and highlighting.
///
/// The injection manager:
/// 1. Tracks detected injection regions (from parent driver's `injections()`)
/// 2. Creates and caches injection layers for embedded languages
/// 3. Produces highlights for embedded content
pub struct InjectionManager {
    /// Cached injection layers by language ID
    layers: HashMap<String, InjectionLayer>,
    /// Optional store for lazy layer creation during highlighting
    layer_store: Option<Arc<InjectionLayerStore>>,
}

impl Default for InjectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl InjectionManager {
    /// Create a new injection manager without a layer store.
    ///
    /// Layers must be pre-registered via [`register_layer()`](Self::register_layer).
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: HashMap::new(),
            layer_store: None,
        }
    }

    /// Create a new injection manager with a layer store for dynamic creation.
    ///
    /// When `highlight_injections()` encounters an unknown language, it queries
    /// the store to create layers on demand.
    #[must_use]
    pub fn with_store(store: Arc<InjectionLayerStore>) -> Self {
        Self {
            layers: HashMap::new(),
            layer_store: Some(store),
        }
    }

    /// Set or replace the layer store for dynamic layer creation.
    pub fn set_store(&mut self, store: Arc<InjectionLayerStore>) {
        self.layer_store = Some(store);
    }

    /// Get or create an injection layer for a language.
    ///
    /// Looks up the layer in the cache first. If not found and a layer store
    /// is configured, queries the store to create one dynamically.
    ///
    /// Returns `None` if the layer is not cached and cannot be created.
    pub fn get_or_create_layer(&mut self, language_id: &str) -> Option<&mut InjectionLayer> {
        if self.layers.contains_key(language_id) {
            return self.layers.get_mut(language_id);
        }

        let store = self.layer_store.as_ref()?;
        let factory = store.find(language_id)?;
        let layer = factory.create_layer()?;
        self.layers.insert(language_id.to_string(), layer);
        self.layers.get_mut(language_id)
    }

    /// Register a pre-created injection layer.
    ///
    /// Call this to add support for an embedded language before highlighting.
    pub fn register_layer(&mut self, layer: InjectionLayer) {
        self.layers.insert(layer.language_id().to_string(), layer);
    }

    /// Check if a layer exists for a language.
    #[must_use]
    pub fn has_layer(&self, language_id: &str) -> bool {
        self.layers.contains_key(language_id)
    }

    /// Get the number of registered layers.
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Highlight all injections that overlap the given byte range.
    ///
    /// Returns highlights from embedded languages, offset-adjusted to parent
    /// document coordinates.
    ///
    /// # Lazy Layer Creation
    ///
    /// If a layer store is configured (via [`with_store()`](Self::with_store) or
    /// [`set_store()`](Self::set_store)), this method lazily creates injection
    /// layers for languages encountered in `injections` that don't already have
    /// a cached layer. Creation happens in a first pass before highlighting to
    /// avoid mutable borrow conflicts.
    pub fn highlight_injections(
        &mut self,
        injections: &[Injection],
        full_content: &str,
        byte_range: Range<usize>,
    ) -> Vec<Annotation> {
        // Phase 1: Lazily create layers for injected languages via store
        if let Some(store) = &self.layer_store {
            for injection in injections {
                if !self.layers.contains_key(&injection.language_id)
                    && let Some(factory) = store.find(&injection.language_id)
                    && let Some(layer) = factory.create_layer()
                {
                    self.layers.insert(injection.language_id.clone(), layer);
                }
            }
        }

        // Phase 2: Highlight with all available layers
        let mut all_highlights = Vec::new();

        for injection in injections {
            // Check if injection overlaps the requested range
            if injection.byte_range.end <= byte_range.start
                || injection.byte_range.start >= byte_range.end
            {
                continue;
            }

            if let Some(layer) = self.layers.get_mut(&injection.language_id) {
                let highlights = layer.highlight_injection(injection, full_content);
                all_highlights.extend(highlights);
            }
        }

        all_highlights
    }

    /// Invalidate all cached parse trees.
    ///
    /// Call this when the parent document content changes significantly.
    pub fn invalidate(&mut self) {
        for layer in self.layers.values_mut() {
            layer.clear_cache();
        }
    }
}

impl std::fmt::Debug for InjectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InjectionManager")
            .field("layer_count", &self.layers.len())
            .field("languages", &self.layers.keys().collect::<Vec<_>>())
            .field("has_store", &self.layer_store.is_some())
            .finish_non_exhaustive()
    }
}

// ============================================================================
// InjectionLayerStore - Global Registry for Layer Factories
// ============================================================================

/// Store for injection layer factories registered by modules during init.
///
/// This follows the same pattern as `SyntaxFactoryStore` but lives in the
/// driver crate to keep tree-sitter types contained.
///
/// # Usage
///
/// Modules register their factories during `init()`:
///
/// ```ignore
/// impl Module for TreesitterRustModule {
///     fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
///         // Register as SyntaxDriverFactory
///         let store = ctx.services.get_or_create::<SyntaxFactoryStore>();
///         store.add(Arc::new(RustSyntaxFactory::new()));
///
///         // Also register as InjectionLayerFactory
///         let injection_store = ctx.services.get_or_create::<InjectionLayerStore>();
///         injection_store.add(Arc::new(RustSyntaxFactory::new()));
///
///         ProbeResult::Success
///     }
/// }
/// ```
///
/// When creating a driver with injection support (e.g., Markdown), query the
/// store to get factories for detected languages:
///
/// ```ignore
/// let store = services.get::<InjectionLayerStore>()?;
/// if let Some(factory) = store.find("rust") {
///     let layer = factory.create_layer()?;
///     manager.register_layer(layer);
/// }
/// ```
#[derive(Default)]
pub struct InjectionLayerStore {
    /// Registered factories (populated during module init).
    factories: parking_lot::RwLock<Vec<Arc<dyn InjectionLayerFactory>>>,
}

impl InjectionLayerStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a factory to the store.
    ///
    /// Called by syntax modules during `init()`.
    pub fn add(&self, factory: Arc<dyn InjectionLayerFactory>) {
        self.factories.write().push(factory);
    }

    /// Find a factory for the given language.
    ///
    /// Returns the first factory that matches the language ID.
    #[must_use]
    pub fn find(&self, language_id: &str) -> Option<Arc<dyn InjectionLayerFactory>> {
        self.factories
            .read()
            .iter()
            .find(|f| f.language_id() == language_id)
            .cloned()
    }

    /// Get all supported language IDs.
    #[must_use]
    pub fn supported_languages(&self) -> Vec<String> {
        self.factories
            .read()
            .iter()
            .map(|f| f.language_id().to_string())
            .collect()
    }

    /// Get the number of registered factories.
    #[must_use]
    pub fn len(&self) -> usize {
        self.factories.read().len()
    }

    /// Check if no factories are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.factories.read().is_empty()
    }
}

/// Implement `Service` trait for `ServiceRegistry` compatibility.
impl reovim_kernel::api::v1::Service for InjectionLayerStore {}

impl std::fmt::Debug for InjectionLayerStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InjectionLayerStore")
            .field("count", &self.len())
            .field("languages", &self.supported_languages())
            .finish()
    }
}

#[cfg(test)]
mod tests {
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
}
