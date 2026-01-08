//! Background completion saturator task
//!
//! Follows the buffer/saturator.rs pattern for non-blocking completion.
//! The saturator runs in a background tokio task and computes completions
//! without blocking the event loop.

use std::sync::Arc;

use {futures::future::join_all, tokio::sync::mpsc};

use nucleo::{
    Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use reovim_core::{
    completion::{CompletionContext, CompletionItem},
    event::RuntimeEvent,
};

use crate::{CompletionCache, cache::CompletionSnapshot, registry::SourceSupport};

/// Request for completion computation
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// Buffer ID
    pub buffer_id: usize,
    /// File path for language detection
    pub file_path: Option<String>,
    /// Buffer content
    pub content: String,
    /// Cursor row (0-indexed)
    pub cursor_row: u32,
    /// Cursor column (0-indexed)
    pub cursor_col: u32,
    /// Current line text
    pub line: String,
    /// Prefix being completed
    pub prefix: String,
    /// Column where word starts
    pub word_start_col: u32,
    /// Optional trigger character
    pub trigger_char: Option<char>,
}

impl CompletionRequest {
    /// Convert to CompletionContext for sources
    #[must_use]
    pub fn to_context(&self) -> CompletionContext {
        let mut ctx = CompletionContext::new(
            self.buffer_id,
            self.cursor_row,
            self.cursor_col,
            self.line.clone(),
            self.prefix.clone(),
            self.word_start_col,
        );
        if let Some(path) = &self.file_path {
            ctx = ctx.with_file_path(path.clone());
        }
        if let Some(ch) = self.trigger_char {
            ctx = ctx.with_trigger_char(ch);
        }
        ctx
    }
}

/// Handle for sending requests to the saturator
#[derive(Debug, Clone)]
pub struct CompletionSaturatorHandle {
    tx: mpsc::Sender<CompletionRequest>,
}

impl CompletionSaturatorHandle {
    /// Request completion computation (non-blocking)
    ///
    /// Uses `try_send` to never block the caller. If the saturator is busy,
    /// the request is dropped (channel buffer = 1).
    pub fn request_completion(&self, request: CompletionRequest) {
        if let Err(e) = self.tx.try_send(request) {
            tracing::debug!("Completion request dropped (saturator busy): {}", e);
        }
    }
}

/// Spawn the completion saturator background task
///
/// # Arguments
/// * `sources` - Arc to the list of completion sources
/// * `cache` - Arc to the completion cache for storing results
/// * `event_tx` - Channel to signal render updates
/// * `max_items` - Maximum number of items to return
///
/// # Returns
/// Handle for sending requests to the saturator
pub fn spawn_completion_saturator(
    sources: Arc<Vec<Arc<dyn SourceSupport>>>,
    cache: Arc<CompletionCache>,
    event_tx: mpsc::Sender<RuntimeEvent>,
    max_items: usize,
) -> CompletionSaturatorHandle {
    // Buffer of 1: only latest request matters
    let (tx, mut rx) = mpsc::channel::<CompletionRequest>(1);

    tokio::spawn(async move {
        tracing::debug!("Completion saturator started");

        while let Some(request) = rx.recv().await {
            tracing::debug!(
                buffer_id = request.buffer_id,
                prefix = %request.prefix,
                "Processing completion request"
            );

            let ctx = request.to_context();

            // Collect available sources
            let available: Vec<_> = sources
                .iter()
                .filter(|s| s.is_available(&ctx))
                .cloned()
                .collect();

            if available.is_empty() {
                tracing::debug!("No available completion sources");
                continue;
            }

            // Query all sources concurrently
            let futures: Vec<_> = available
                .iter()
                .map(|source| {
                    let ctx = ctx.clone();
                    let content = request.content.clone();
                    async move { source.complete(&ctx, &content).await }
                })
                .collect();

            let results: Vec<Vec<CompletionItem>> = join_all(futures).await;

            // Merge results
            let mut items: Vec<CompletionItem> = results.into_iter().flatten().collect();

            // Filter and score using nucleo fuzzy matching if prefix is non-empty
            let prefix = &request.prefix;
            if !prefix.is_empty() {
                let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
                let pattern = Pattern::new(
                    prefix,
                    CaseMatching::Smart,
                    Normalization::Smart,
                    AtomKind::Fuzzy,
                );

                // Minimum score threshold: require reasonable match quality
                // Score roughly scales with match quality - require at least
                // some portion of the prefix to match well
                let min_score = (prefix.len() as u32).saturating_mul(10);

                items = items
                    .into_iter()
                    .filter_map(|mut item| {
                        let filter_text = item.filter_text();
                        let mut buf = Vec::new();
                        let haystack = Utf32Str::new(filter_text, &mut buf);
                        let mut indices = Vec::new();

                        pattern
                            .indices(haystack, &mut matcher, &mut indices)
                            .filter(|&score| score >= min_score)
                            .map(|score| {
                                item.score = score;
                                item.match_indices = indices.to_vec();
                                item
                            })
                    })
                    .collect();
            }

            // Sort by priority, then score (descending), then label
            items.sort_by(|a, b| {
                a.sort_priority
                    .cmp(&b.sort_priority)
                    .then_with(|| b.score.cmp(&a.score))
                    .then_with(|| a.label.cmp(&b.label))
            });

            // Limit results
            items.truncate(max_items);

            let item_count = items.len();

            // Create and store snapshot
            let snapshot = CompletionSnapshot::new(
                items,
                request.prefix.clone(),
                request.buffer_id,
                request.cursor_row,
                request.cursor_col,
                request.word_start_col,
            );
            cache.store(snapshot);

            tracing::debug!(item_count, "Completion results ready");

            // Signal render update
            if let Err(e) = event_tx.send(RuntimeEvent::render_signal()).await {
                tracing::warn!("Failed to send render signal: {}", e);
            }
        }

        tracing::debug!("Completion saturator stopped");
    });

    CompletionSaturatorHandle { tx }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{future::Future, pin::Pin},
    };

    struct TestSource {
        items: Vec<&'static str>,
    }

    impl SourceSupport for TestSource {
        fn source_id(&self) -> &'static str {
            "test"
        }

        fn complete<'a>(
            &'a self,
            _ctx: &'a CompletionContext,
            _content: &'a str,
        ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
            let items: Vec<_> = self
                .items
                .iter()
                .map(|s| CompletionItem::new(*s, "test"))
                .collect();
            Box::pin(async move { items })
        }
    }

    #[allow(dead_code)] // May be useful for future priority-based tests
    struct PrioritySource {
        id: &'static str,
        priority: u32,
        items: Vec<&'static str>,
    }

    impl SourceSupport for PrioritySource {
        fn source_id(&self) -> &'static str {
            self.id
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        fn complete<'a>(
            &'a self,
            _ctx: &'a CompletionContext,
            _content: &'a str,
        ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
            let items: Vec<_> = self
                .items
                .iter()
                .map(|s| CompletionItem::new(*s, self.id).with_priority(self.priority))
                .collect();
            Box::pin(async move { items })
        }
    }

    struct UnavailableSource;

    impl SourceSupport for UnavailableSource {
        fn source_id(&self) -> &'static str {
            "unavailable"
        }

        fn is_available(&self, _ctx: &CompletionContext) -> bool {
            false
        }

        fn complete<'a>(
            &'a self,
            _ctx: &'a CompletionContext,
            _content: &'a str,
        ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
            Box::pin(async move { vec![CompletionItem::new("should_not_appear", "unavailable")] })
        }
    }

    fn make_request(prefix: &str) -> CompletionRequest {
        CompletionRequest {
            buffer_id: 1,
            file_path: None,
            content: "test content".to_string(),
            cursor_row: 0,
            cursor_col: prefix.len() as u32,
            line: prefix.to_string(),
            prefix: prefix.to_string(),
            word_start_col: 0,
            trigger_char: None,
        }
    }

    #[tokio::test]
    async fn test_saturator_basic() {
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![Arc::new(TestSource {
            items: vec!["foo", "bar", "baz"],
        })]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        // Empty prefix shows all items (no filtering)
        handle.request_completion(make_request(""));

        // Wait for render signal
        let event = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert!(matches!(
            event.into_payload(),
            reovim_core::event::RuntimeEventPayload::Render(
                reovim_core::event::RenderEvent::Signal
            )
        ));

        // Check cache
        let snapshot = cache.load();
        assert!(snapshot.active);
        assert_eq!(snapshot.items.len(), 3);
    }

    #[tokio::test]
    async fn test_saturator_multiple_sources() {
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![
            Arc::new(TestSource {
                items: vec!["alpha", "apex"],
            }),
            Arc::new(TestSource {
                items: vec!["beta", "bravo"],
            }),
        ]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request(""));

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.items.len(), 4);
    }

    #[tokio::test]
    async fn test_saturator_max_items_limit() {
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![Arc::new(TestSource {
            items: vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"],
        })]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        // Limit to 5 items
        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 5);

        handle.request_completion(make_request(""));

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.items.len(), 5);
    }

    #[tokio::test]
    async fn test_saturator_skips_unavailable_sources() {
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![
            Arc::new(TestSource {
                items: vec!["visible"],
            }),
            Arc::new(UnavailableSource),
        ]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request(""));

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].label, "visible");
    }

    #[tokio::test]
    async fn test_saturator_empty_sources() {
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request("test"));

        // Should timeout since no sources means no completion
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.recv()).await;

        assert!(result.is_err()); // Timeout expected
    }

    #[tokio::test]
    async fn test_saturator_stores_context_info() {
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![Arc::new(TestSource {
            items: vec!["test"],
        })]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        let request = CompletionRequest {
            buffer_id: 42,
            file_path: Some("/path/to/file.rs".to_string()),
            content: "let x = ".to_string(),
            cursor_row: 10,
            cursor_col: 8,
            line: "let x = ".to_string(),
            prefix: "x".to_string(),
            word_start_col: 4,
            trigger_char: Some('.'),
        };

        handle.request_completion(request);

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.buffer_id, 42);
        assert_eq!(snapshot.cursor_row, 10);
        assert_eq!(snapshot.cursor_col, 8);
        assert_eq!(snapshot.word_start_col, 4);
        assert_eq!(snapshot.prefix, "x");
    }

    #[tokio::test]
    async fn test_request_to_context_conversion() {
        let request = CompletionRequest {
            buffer_id: 1,
            file_path: Some("/test.rs".to_string()),
            content: "hello".to_string(),
            cursor_row: 5,
            cursor_col: 10,
            line: "let x = ".to_string(),
            prefix: "pre".to_string(),
            word_start_col: 7,
            trigger_char: Some('.'),
        };

        let ctx = request.to_context();

        assert_eq!(ctx.buffer_id, 1);
        assert_eq!(ctx.file_path, Some("/test.rs".to_string()));
        assert_eq!(ctx.cursor_row, 5);
        assert_eq!(ctx.cursor_col, 10);
        assert_eq!(ctx.line, "let x = ");
        assert_eq!(ctx.prefix, "pre");
        assert_eq!(ctx.word_start_col, 7);
        assert_eq!(ctx.trigger_char, Some('.'));
    }

    #[tokio::test]
    async fn test_request_to_context_without_optional_fields() {
        let request = CompletionRequest {
            buffer_id: 1,
            file_path: None,
            content: "test".to_string(),
            cursor_row: 0,
            cursor_col: 0,
            line: String::new(),
            prefix: String::new(),
            word_start_col: 0,
            trigger_char: None,
        };

        let ctx = request.to_context();

        assert!(ctx.file_path.is_none());
        assert!(ctx.trigger_char.is_none());
    }

    #[test]
    fn test_saturator_handle_clone() {
        // Handle should be cloneable
        let (tx, _rx) = mpsc::channel::<CompletionRequest>(1);
        let handle = CompletionSaturatorHandle { tx };
        let _cloned = handle.clone();
    }

    #[test]
    fn test_completion_request_debug() {
        let request = make_request("test");
        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("CompletionRequest"));
        assert!(debug_str.contains("buffer_id"));
    }

    #[tokio::test]
    async fn test_saturator_priority_sorting() {
        // Test that items are sorted by priority (lower = higher priority)
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![
            Arc::new(PrioritySource {
                id: "low",
                priority: 200, // Low priority
                items: vec!["low_item"],
            }),
            Arc::new(PrioritySource {
                id: "high",
                priority: 50, // High priority
                items: vec!["high_item"],
            }),
            Arc::new(PrioritySource {
                id: "medium",
                priority: 100, // Medium priority
                items: vec!["medium_item"],
            }),
        ]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request(""));

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.items.len(), 3);

        // Should be sorted by priority: high (50), medium (100), low (200)
        assert_eq!(snapshot.items[0].label, "high_item");
        assert_eq!(snapshot.items[1].label, "medium_item");
        assert_eq!(snapshot.items[2].label, "low_item");
    }

    #[tokio::test]
    async fn test_saturator_score_sorting() {
        // Test that items with same priority are sorted by score
        struct ScoredSource;

        impl SourceSupport for ScoredSource {
            fn source_id(&self) -> &'static str {
                "scored"
            }

            fn complete<'a>(
                &'a self,
                _ctx: &'a CompletionContext,
                _content: &'a str,
            ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
                Box::pin(async move {
                    let mut low = CompletionItem::new("low_score", "scored");
                    low.score = 10;
                    let mut high = CompletionItem::new("high_score", "scored");
                    high.score = 100;
                    let mut medium = CompletionItem::new("medium_score", "scored");
                    medium.score = 50;
                    vec![low, high, medium]
                })
            }
        }

        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![Arc::new(ScoredSource)]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request(""));

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.items.len(), 3);

        // Should be sorted by score descending: high (100), medium (50), low (10)
        assert_eq!(snapshot.items[0].label, "high_score");
        assert_eq!(snapshot.items[1].label, "medium_score");
        assert_eq!(snapshot.items[2].label, "low_score");
    }

    #[tokio::test]
    async fn test_saturator_alphabetical_tiebreaker() {
        // Test that items with same priority and score are sorted alphabetically
        struct AlphaSource;

        impl SourceSupport for AlphaSource {
            fn source_id(&self) -> &'static str {
                "alpha"
            }

            fn complete<'a>(
                &'a self,
                _ctx: &'a CompletionContext,
                _content: &'a str,
            ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
                Box::pin(async move {
                    vec![
                        CompletionItem::new("zebra", "alpha"),
                        CompletionItem::new("apple", "alpha"),
                        CompletionItem::new("mango", "alpha"),
                    ]
                })
            }
        }

        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![Arc::new(AlphaSource)]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request(""));

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.items.len(), 3);

        // Should be sorted alphabetically: apple, mango, zebra
        assert_eq!(snapshot.items[0].label, "apple");
        assert_eq!(snapshot.items[1].label, "mango");
        assert_eq!(snapshot.items[2].label, "zebra");
    }

    #[tokio::test]
    async fn test_saturator_request_dropping() {
        // Test that when saturator is busy, only the latest request is processed
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct SlowSource {
            call_count: Arc<AtomicUsize>,
        }

        impl SourceSupport for SlowSource {
            fn source_id(&self) -> &'static str {
                "slow"
            }

            fn complete<'a>(
                &'a self,
                _ctx: &'a CompletionContext,
                _content: &'a str,
            ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
                let count = Arc::clone(&self.call_count);
                Box::pin(async move {
                    // Simulate slow processing
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    count.fetch_add(1, Ordering::SeqCst);
                    vec![CompletionItem::new("slow_result", "slow")]
                })
            }
        }

        let call_count = Arc::new(AtomicUsize::new(0));
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![Arc::new(SlowSource {
            call_count: Arc::clone(&call_count),
        })]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        // Send multiple requests rapidly (channel buffer = 1, so some should be dropped)
        for i in 0..5 {
            let mut req = make_request("test");
            req.buffer_id = i;
            handle.request_completion(req);
        }

        // Wait for processing to complete
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Drain all events
        while let Ok(Some(_)) =
            tokio::time::timeout(std::time::Duration::from_millis(10), event_rx.recv()).await
        {
            // Consume events
        }

        // Due to channel buffer of 1 and slow processing, not all 5 requests should be processed
        let count = call_count.load(Ordering::SeqCst);
        assert!(count < 5, "Expected fewer than 5 calls due to request dropping, got {}", count);
        assert!(count >= 1, "Expected at least 1 call, got {}", count);
    }

    #[tokio::test]
    async fn test_saturator_all_sources_unavailable() {
        // Test behavior when all sources return is_available = false
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> =
            Arc::new(vec![Arc::new(UnavailableSource), Arc::new(UnavailableSource)]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request("test"));

        // Should timeout since no available sources
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.recv()).await;

        assert!(result.is_err()); // Timeout expected
        assert!(!cache.is_active()); // Cache should remain inactive
    }

    #[tokio::test]
    async fn test_saturator_handles_source_returning_empty() {
        // Test that sources returning empty vec don't break the flow
        struct EmptySource;

        impl SourceSupport for EmptySource {
            fn source_id(&self) -> &'static str {
                "empty"
            }

            fn complete<'a>(
                &'a self,
                _ctx: &'a CompletionContext,
                _content: &'a str,
            ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
                Box::pin(async move { vec![] })
            }
        }

        let sources: Arc<Vec<Arc<dyn SourceSupport>>> = Arc::new(vec![
            Arc::new(EmptySource),
            Arc::new(TestSource {
                items: vec!["valid"],
            }),
        ]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        handle.request_completion(make_request(""));

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        let snapshot = cache.load();
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].label, "valid");
    }

    #[tokio::test]
    async fn test_saturator_preserves_trigger_char_in_context() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct TriggerCheckSource {
            trigger_seen: Arc<AtomicBool>,
        }

        impl SourceSupport for TriggerCheckSource {
            fn source_id(&self) -> &'static str {
                "trigger_check"
            }

            fn complete<'a>(
                &'a self,
                ctx: &'a CompletionContext,
                _content: &'a str,
            ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
                let trigger_seen = Arc::clone(&self.trigger_seen);
                Box::pin(async move {
                    if ctx.trigger_char == Some('.') {
                        trigger_seen.store(true, Ordering::SeqCst);
                    }
                    vec![CompletionItem::new("result", "trigger_check")]
                })
            }
        }

        let trigger_seen = Arc::new(AtomicBool::new(false));
        let sources: Arc<Vec<Arc<dyn SourceSupport>>> =
            Arc::new(vec![Arc::new(TriggerCheckSource {
                trigger_seen: Arc::clone(&trigger_seen),
            })]);
        let cache = Arc::new(CompletionCache::new());
        let (event_tx, mut event_rx) = mpsc::channel(10);

        let handle =
            spawn_completion_saturator(Arc::clone(&sources), Arc::clone(&cache), event_tx, 100);

        let mut request = make_request("obj.");
        request.trigger_char = Some('.');
        handle.request_completion(request);

        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv())
            .await
            .expect("timeout");

        assert!(trigger_seen.load(Ordering::SeqCst));
    }
}
