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
    reovim_driver_syntax::{HighlightSpan, Injection, SyntaxDriverFactory},
    tree_sitter::{Parser, Query, Tree},
};

use crate::CaptureMapper;

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
    /// Capture mapper for converting captures to `HighlightGroup`
    capture_mapper: Arc<CaptureMapper>,
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
        capture_mapper: Arc<CaptureMapper>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(ts_language).ok()?;

        Some(Self {
            language_id: language_id.to_string(),
            parser: Mutex::new(parser),
            trees: HashMap::new(),
            highlight_query,
            capture_mapper,
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
    pub fn highlight_injection(
        &mut self,
        injection: &Injection,
        full_content: &str,
    ) -> Vec<HighlightSpan> {
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

                let group = self.capture_mapper.map(capture_name);
                let node = capture.node;

                // Offset byte positions to parent document coordinates
                let start_byte = range.start + node.start_byte();
                let end_byte = range.start + node.end_byte();

                highlights.push(HighlightSpan::new(start_byte, end_byte, group));
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
///     fn create_layer(&self, capture_mapper: Arc<CaptureMapper>) -> Option<InjectionLayer> {
///         let language = tree_sitter_rust::LANGUAGE;
///         InjectionLayer::new("rust", &language.into(), self.highlight_query.clone(), capture_mapper)
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
    fn create_layer(&self, capture_mapper: Arc<CaptureMapper>) -> Option<InjectionLayer>;

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
    /// Shared capture mapper for creating layers
    capture_mapper: Arc<CaptureMapper>,
}

impl InjectionManager {
    /// Create a new injection manager.
    #[must_use]
    pub fn new(capture_mapper: Arc<CaptureMapper>) -> Self {
        Self {
            layers: HashMap::new(),
            capture_mapper,
        }
    }

    /// Get or create an injection layer for a language.
    ///
    /// Uses the provided factory to create new layers. Returns `None` if the
    /// factory doesn't support the language.
    pub fn get_or_create_layer(
        &mut self,
        language_id: &str,
        factory: &dyn SyntaxDriverFactory,
    ) -> Option<&mut InjectionLayer> {
        if self.layers.contains_key(language_id) {
            return self.layers.get_mut(language_id);
        }

        // Try to create a layer using the factory.
        // Note: Dynamic layer creation requires InjectionLayerFactory implementations
        // to be registered in InjectionLayerStore during module init(). Languages not
        // pre-registered will be skipped.
        //
        // See: Phase 12.4 added InjectionLayerFactory to RustSyntaxFactory and MarkdownSyntaxFactory
        tracing::debug!(
            language_id = %language_id,
            "Injection layer requested but factory-based creation not yet implemented"
        );

        // Check if factory supports the language (for logging)
        if !factory.supports(language_id) {
            tracing::debug!(
                language_id = %language_id,
                "Injection layer skipped: language not supported by factory"
            );
        }

        None
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
    pub fn highlight_injections(
        &mut self,
        injections: &[Injection],
        full_content: &str,
        byte_range: Range<usize>,
    ) -> Vec<HighlightSpan> {
        let mut all_highlights = Vec::new();

        for injection in injections {
            // Check if injection overlaps the requested range
            if injection.byte_range.end <= byte_range.start
                || injection.byte_range.start >= byte_range.end
            {
                continue;
            }

            // Get layer for this language
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

    /// Get the shared capture mapper.
    #[must_use]
    pub const fn capture_mapper(&self) -> &Arc<CaptureMapper> {
        &self.capture_mapper
    }
}

impl std::fmt::Debug for InjectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InjectionManager")
            .field("layer_count", &self.layers.len())
            .field("languages", &self.layers.keys().collect::<Vec<_>>())
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
///     let layer = factory.create_layer(capture_mapper.clone())?;
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
    fn test_injection_manager_new() {
        let mapper = Arc::new(CaptureMapper::new());
        let manager = InjectionManager::new(mapper);
        assert_eq!(manager.layer_count(), 0);
    }

    #[test]
    fn test_injection_manager_has_layer() {
        let mapper = Arc::new(CaptureMapper::new());
        let manager = InjectionManager::new(mapper);
        assert!(!manager.has_layer("rust"));
    }

    #[test]
    fn test_injection_manager_debug() {
        let mapper = Arc::new(CaptureMapper::new());
        let manager = InjectionManager::new(mapper);
        let debug = format!("{manager:?}");
        assert!(debug.contains("InjectionManager"));
        assert!(debug.contains("layer_count"));
    }

    #[test]
    fn test_injection_manager_invalidate() {
        let mapper = Arc::new(CaptureMapper::new());
        let mut manager = InjectionManager::new(mapper);

        // Invalidate should not panic even with no layers
        manager.invalidate();
        assert_eq!(manager.layer_count(), 0);
    }

    #[test]
    fn test_injection_manager_highlight_injections_empty() {
        let mapper = Arc::new(CaptureMapper::new());
        let mut manager = InjectionManager::new(mapper);

        let injections: Vec<Injection> = vec![];
        let highlights = manager.highlight_injections(&injections, "content", 0..100);

        assert!(highlights.is_empty());
    }

    #[test]
    fn test_injection_manager_highlight_injections_no_layer() {
        let mapper = Arc::new(CaptureMapper::new());
        let mut manager = InjectionManager::new(mapper);

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

        let mapper = Arc::new(CaptureMapper::new());
        let layer = InjectionLayer::new("rust", &language, Arc::new(query), mapper);

        assert!(layer.is_some());
        let layer = layer.unwrap();
        assert_eq!(layer.language_id(), "rust");
    }

    #[test]
    fn test_injection_layer_highlight() {
        // Test highlighting embedded Rust code
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();

        // Create a query that matches identifiers
        let query = Query::new(&language, "(identifier) @variable").unwrap();

        let mapper = Arc::new(CaptureMapper::new());
        let mut layer = InjectionLayer::new("rust", &language, Arc::new(query), mapper).unwrap();

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
    fn test_injection_manager_with_layer() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let query = Query::new(&language, "(identifier) @variable").unwrap();

        let mapper = Arc::new(CaptureMapper::new());
        let mut manager = InjectionManager::new(Arc::clone(&mapper));

        // Register a Rust layer
        let layer = InjectionLayer::new("rust", &language, Arc::new(query), mapper).unwrap();
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

    /// Mock factory for testing the store.
    struct MockLayerFactory {
        language: &'static str,
    }

    impl InjectionLayerFactory for MockLayerFactory {
        fn create_layer(&self, _capture_mapper: Arc<CaptureMapper>) -> Option<InjectionLayer> {
            // For testing, we don't actually create a layer
            None
        }

        fn language_id(&self) -> &'static str {
            self.language
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
}
