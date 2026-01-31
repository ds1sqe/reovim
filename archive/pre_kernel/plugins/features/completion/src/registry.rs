//! Source registry and SourceSupport trait
//!
//! Following the treesitter LanguageRegistry pattern, this module provides
//! the trait that completion sources implement and the registry that manages them.

use std::{future::Future, pin::Pin, sync::Arc};

use reovim_core::completion::{CompletionContext, CompletionItem};

/// Trait for completion source implementations
///
/// External plugins implement this trait to provide completions.
/// Similar to treesitter's `LanguageSupport` trait.
///
/// # Example
///
/// ```ignore
/// struct LspCompletionSource { client: LspClient }
///
/// impl SourceSupport for LspCompletionSource {
///     fn source_id(&self) -> &'static str { "lsp" }
///     fn priority(&self) -> u32 { 50 }  // Higher priority than buffer
///
///     fn complete(&self, ctx: &CompletionContext, content: &str)
///         -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + '_>>
///     {
///         Box::pin(async move {
///             self.client.request_completion(ctx).await
///         })
///     }
/// }
/// ```
pub trait SourceSupport: Send + Sync + 'static {
    /// Unique identifier for this source (e.g., "buffer", "lsp", "path")
    fn source_id(&self) -> &'static str;

    /// Priority for sorting results (lower = higher priority)
    ///
    /// Default is 100. LSP sources typically use 50, buffer sources use 100.
    fn priority(&self) -> u32 {
        100
    }

    /// Check if this source is available for the given context
    ///
    /// Called before `complete()` to filter out unavailable sources.
    fn is_available(&self, _ctx: &CompletionContext) -> bool {
        true
    }

    /// Fetch completions asynchronously
    ///
    /// Called by the saturator in a background task.
    fn complete<'a>(
        &'a self,
        ctx: &'a CompletionContext,
        content: &'a str,
    ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>>;

    /// Optional: Resolve additional details for a completion item
    ///
    /// Called when an item is selected to fetch documentation, etc.
    fn resolve<'a>(
        &'a self,
        item: &'a CompletionItem,
    ) -> Pin<Box<dyn Future<Output = CompletionItem> + Send + 'a>> {
        let item = item.clone();
        Box::pin(async move { item })
    }

    /// Optional: Execute post-selection action
    ///
    /// Called after a completion is confirmed (e.g., to add imports).
    fn execute(&self, _item: &CompletionItem) {}

    /// Optional: Characters that trigger completion immediately
    ///
    /// For example, LSP might return ['.', ':', '<'] for Rust.
    fn trigger_characters(&self) -> Option<&[char]> {
        None
    }
}

/// Registry of completion sources
///
/// Manages registered sources and provides access by ID.
#[derive(Default)]
pub struct SourceRegistry {
    sources: Vec<Arc<dyn SourceSupport>>,
}

impl SourceRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Register a completion source
    ///
    /// Sources are stored in priority order (lower priority value = earlier).
    pub fn register(&mut self, source: Arc<dyn SourceSupport>) {
        tracing::info!(
            source_id = source.source_id(),
            priority = source.priority(),
            "Registered completion source"
        );

        // Insert in priority order
        let priority = source.priority();
        let pos = self
            .sources
            .iter()
            .position(|s| s.priority() > priority)
            .unwrap_or(self.sources.len());
        self.sources.insert(pos, source);
    }

    /// Get all registered sources
    #[must_use]
    pub fn sources(&self) -> &[Arc<dyn SourceSupport>] {
        &self.sources
    }

    /// Get available sources for a context
    pub fn available_sources(&self, ctx: &CompletionContext) -> Vec<Arc<dyn SourceSupport>> {
        self.sources
            .iter()
            .filter(|s| s.is_available(ctx))
            .cloned()
            .collect()
    }

    /// Get a source by ID
    #[must_use]
    pub fn get(&self, source_id: &str) -> Option<Arc<dyn SourceSupport>> {
        self.sources
            .iter()
            .find(|s| s.source_id() == source_id)
            .cloned()
    }

    /// Get all trigger characters from all sources
    #[must_use]
    pub fn all_trigger_characters(&self) -> Vec<char> {
        self.sources
            .iter()
            .filter_map(|s| s.trigger_characters())
            .flatten()
            .copied()
            .collect()
    }

    /// Number of registered sources
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Check if registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

impl std::fmt::Debug for SourceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceRegistry")
            .field(
                "sources",
                &self
                    .sources
                    .iter()
                    .map(|s| s.source_id())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test source with configurable priority and trigger characters
    struct TestSource {
        id: &'static str,
        priority: u32,
        trigger_chars: Option<&'static [char]>,
        available: bool,
    }

    impl TestSource {
        fn new(id: &'static str) -> Self {
            Self {
                id,
                priority: 100,
                trigger_chars: None,
                available: true,
            }
        }

        fn with_priority(mut self, priority: u32) -> Self {
            self.priority = priority;
            self
        }

        fn with_trigger_chars(mut self, chars: &'static [char]) -> Self {
            self.trigger_chars = Some(chars);
            self
        }

        fn unavailable(mut self) -> Self {
            self.available = false;
            self
        }
    }

    impl SourceSupport for TestSource {
        fn source_id(&self) -> &'static str {
            self.id
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        fn is_available(&self, _ctx: &CompletionContext) -> bool {
            self.available
        }

        fn trigger_characters(&self) -> Option<&[char]> {
            self.trigger_chars
        }

        fn complete<'a>(
            &'a self,
            _ctx: &'a CompletionContext,
            _content: &'a str,
        ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
            Box::pin(async move { vec![CompletionItem::new("test", self.id)] })
        }
    }

    fn make_context() -> CompletionContext {
        CompletionContext::new(0, 0, 0, String::new(), String::new(), 0)
    }

    #[test]
    fn test_registry_new_is_empty() {
        let registry = SourceRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register_source() {
        let mut registry = SourceRegistry::new();
        registry.register(Arc::new(TestSource::new("test1")));

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_get_by_id() {
        let mut registry = SourceRegistry::new();
        registry.register(Arc::new(TestSource::new("source_a")));
        registry.register(Arc::new(TestSource::new("source_b")));

        let source = registry.get("source_a");
        assert!(source.is_some());
        assert_eq!(source.unwrap().source_id(), "source_a");

        let missing = registry.get("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_registry_priority_ordering() {
        let mut registry = SourceRegistry::new();

        // Register in random order
        registry.register(Arc::new(TestSource::new("low").with_priority(200)));
        registry.register(Arc::new(TestSource::new("high").with_priority(10)));
        registry.register(Arc::new(TestSource::new("medium").with_priority(100)));

        // Should be sorted by priority (ascending)
        let sources = registry.sources();
        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0].source_id(), "high");
        assert_eq!(sources[1].source_id(), "medium");
        assert_eq!(sources[2].source_id(), "low");
    }

    #[test]
    fn test_registry_available_sources() {
        let mut registry = SourceRegistry::new();
        registry.register(Arc::new(TestSource::new("available1")));
        registry.register(Arc::new(TestSource::new("unavailable").unavailable()));
        registry.register(Arc::new(TestSource::new("available2")));

        let ctx = make_context();
        let available = registry.available_sources(&ctx);

        assert_eq!(available.len(), 2);
        let ids: Vec<_> = available.iter().map(|s| s.source_id()).collect();
        assert!(ids.contains(&"available1"));
        assert!(ids.contains(&"available2"));
        assert!(!ids.contains(&"unavailable"));
    }

    #[test]
    fn test_registry_all_trigger_characters() {
        let mut registry = SourceRegistry::new();
        registry.register(Arc::new(TestSource::new("source1").with_trigger_chars(&['.', ':'])));
        registry.register(Arc::new(TestSource::new("source2"))); // No triggers
        registry.register(Arc::new(TestSource::new("source3").with_trigger_chars(&['/', '<'])));

        let triggers = registry.all_trigger_characters();

        assert_eq!(triggers.len(), 4);
        assert!(triggers.contains(&'.'));
        assert!(triggers.contains(&':'));
        assert!(triggers.contains(&'/'));
        assert!(triggers.contains(&'<'));
    }

    #[test]
    fn test_registry_sources_returns_all() {
        let mut registry = SourceRegistry::new();
        registry.register(Arc::new(TestSource::new("a")));
        registry.register(Arc::new(TestSource::new("b")));
        registry.register(Arc::new(TestSource::new("c")));

        let sources = registry.sources();
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn test_registry_debug_format() {
        let mut registry = SourceRegistry::new();
        registry.register(Arc::new(TestSource::new("test_source")));

        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("SourceRegistry"));
        assert!(debug_str.contains("test_source"));
    }

    #[test]
    fn test_source_default_implementations() {
        struct MinimalSource;

        impl SourceSupport for MinimalSource {
            fn source_id(&self) -> &'static str {
                "minimal"
            }

            fn complete<'a>(
                &'a self,
                _ctx: &'a CompletionContext,
                _content: &'a str,
            ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
                Box::pin(async move { vec![] })
            }
        }

        let source = MinimalSource;
        let ctx = make_context();

        // Test default implementations
        assert_eq!(source.priority(), 100);
        assert!(source.is_available(&ctx));
        assert!(source.trigger_characters().is_none());
    }

    #[tokio::test]
    async fn test_source_resolve_default() {
        struct SimpleSource;

        impl SourceSupport for SimpleSource {
            fn source_id(&self) -> &'static str {
                "simple"
            }

            fn complete<'a>(
                &'a self,
                _ctx: &'a CompletionContext,
                _content: &'a str,
            ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
                Box::pin(async move { vec![] })
            }
        }

        let source = SimpleSource;
        let item = CompletionItem::new("test", "simple");

        // Default resolve returns the same item
        let resolved = source.resolve(&item).await;
        assert_eq!(resolved.label, item.label);
        assert_eq!(resolved.source, item.source);
    }
}
