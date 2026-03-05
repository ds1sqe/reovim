//! Streaming fuzzy matching engine for completion items, powered by nucleo.
//!
//! Wraps `Nucleo` to provide a simple API for the completion system.
//! Items are fed via a lock-free `Injector`, and results are polled
//! via non-blocking `tick()` + `matched_items()`.

use std::sync::Arc;

use nucleo::{
    Config, Injector, Nucleo, Utf32String,
    pattern::{CaseMatching, Normalization},
};

use crate::CompletionItem;

/// A single item stored in the nucleo engine.
#[derive(Clone)]
pub struct EngineItem {
    /// The original completion item.
    pub item: CompletionItem,
    /// Pre-converted text for nucleo matching.
    match_text: Utf32String,
}

/// Status returned by [`CompletionEngine::tick`].
#[derive(Debug, Clone, Copy)]
pub struct TickStatus {
    /// Whether nucleo worker threads are still processing.
    pub running: bool,
    /// Whether the result set changed since the last tick.
    pub changed: bool,
}

/// Streaming fuzzy matching engine for completions.
///
/// Items are injected via the lock-free [`Injector`] returned by
/// [`injector()`](Self::injector). Results are polled by calling
/// [`tick()`](Self::tick) followed by [`matched_items()`](Self::matched_items).
pub struct CompletionEngine {
    nucleo: Nucleo<EngineItem>,
}

impl CompletionEngine {
    /// Create a new engine with default configuration.
    #[must_use]
    pub fn new() -> Self {
        let config = Config::DEFAULT;
        let notify = Arc::new(|| {});
        let nucleo = Nucleo::new(config, notify, None, 1);
        Self { nucleo }
    }

    /// Get a lock-free injector for pushing items from any thread.
    #[must_use]
    pub fn injector(&self) -> Injector<EngineItem> {
        self.nucleo.injector()
    }

    /// Update the fuzzy matching pattern.
    pub fn set_pattern(&mut self, query: &str) {
        self.nucleo.pattern.reparse(
            0,
            query,
            CaseMatching::Smart,
            Normalization::Smart,
            query.starts_with(char::is_lowercase),
        );
    }

    /// Non-blocking tick. Drives the matching worker threads.
    ///
    /// Call this regularly. Use `timeout_ms = 0` for non-blocking poll,
    /// or a small value like 10 for periodic ticks.
    pub fn tick(&mut self, timeout_ms: u64) -> TickStatus {
        let status = self.nucleo.tick(timeout_ms);
        TickStatus {
            running: status.running,
            changed: status.changed,
        }
    }

    /// Get the top matched items, up to `max`.
    ///
    /// Items are returned in score-descending order (best match first).
    #[must_use]
    pub fn matched_items(&self, max: usize) -> Vec<CompletionItem> {
        let snapshot = self.nucleo.snapshot();
        let limit = u32::try_from(max)
            .unwrap_or(u32::MAX)
            .min(snapshot.matched_item_count());
        snapshot
            .matched_items(..limit)
            .map(|mi| mi.data.item.clone())
            .collect()
    }

    /// Total number of items in the engine (matched + unmatched).
    #[must_use]
    pub fn total_count(&self) -> u32 {
        self.nucleo.snapshot().item_count()
    }

    /// Number of items matching the current pattern.
    #[must_use]
    pub fn matched_count(&self) -> u32 {
        self.nucleo.snapshot().matched_item_count()
    }

    /// Whether the current pattern is empty (all items match).
    #[must_use]
    pub fn is_pattern_empty(&self) -> bool {
        self.nucleo.pattern.is_empty()
    }

    /// Clear all items and reset the engine.
    pub fn restart(&mut self) {
        self.nucleo.restart(false);
    }
}

impl Default for CompletionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Push a single completion item into the engine via an injector.
pub fn push_item(injector: &Injector<EngineItem>, item: CompletionItem) {
    let match_text = Utf32String::from(item.label.as_str());
    let engine_item = EngineItem { item, match_text };
    let _ = injector.push(engine_item, |ei, cols| {
        cols[0] = ei.match_text.clone();
    });
}

/// Push multiple completion items into the engine via an injector.
pub fn push_items(
    injector: &Injector<EngineItem>,
    items: impl IntoIterator<Item = CompletionItem>,
) {
    for item in items {
        push_item(injector, item);
    }
}

#[cfg(test)]
mod tests {
    use crate::CompletionKind;

    use super::*;

    fn make_item(label: &str) -> CompletionItem {
        CompletionItem {
            label: label.to_string(),
            insert_text: label.to_string(),
            kind: CompletionKind::Text,
            detail: None,
            documentation: None,
            source_id: "test",
            is_snippet: false,
            sort_priority: 100,
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn settle(engine: &mut CompletionEngine) {
        for _ in 0..100 {
            let status = engine.tick(10);
            if !status.running {
                break;
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn inject_and_settle(engine: &mut CompletionEngine, items: Vec<CompletionItem>) {
        let injector = engine.injector();
        push_items(&injector, items);
        settle(engine);
    }

    #[test]
    fn empty_engine() {
        let engine = CompletionEngine::new();
        assert_eq!(engine.total_count(), 0);
        assert_eq!(engine.matched_count(), 0);
        assert!(engine.is_pattern_empty());
        assert!(engine.matched_items(10).is_empty());
    }

    #[test]
    fn default_engine() {
        let engine = CompletionEngine::default();
        assert_eq!(engine.total_count(), 0);
    }

    #[test]
    fn inject_and_count() {
        let mut engine = CompletionEngine::new();
        let items = vec![make_item("alpha"), make_item("beta"), make_item("gamma")];
        inject_and_settle(&mut engine, items);

        assert_eq!(engine.total_count(), 3);
        assert_eq!(engine.matched_count(), 3);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn pattern_filtering() {
        let mut engine = CompletionEngine::new();
        let items = vec![
            make_item("function_name"),
            make_item("variable_name"),
            make_item("constant_value"),
            make_item("func_helper"),
        ];
        inject_and_settle(&mut engine, items);

        engine.set_pattern("func");
        settle(&mut engine);

        assert!(!engine.is_pattern_empty());
        let matched = engine.matched_items(10);
        assert!(matched.len() >= 2);
        for item in &matched {
            assert!(
                item.label.contains("func") || item.label.contains("fun"),
                "Expected item to match 'func', got: {}",
                item.label
            );
        }
    }

    #[test]
    fn matched_items_respects_max() {
        let mut engine = CompletionEngine::new();
        let items: Vec<CompletionItem> = (0..20).map(|i| make_item(&format!("item_{i}"))).collect();
        inject_and_settle(&mut engine, items);

        let matched = engine.matched_items(5);
        assert!(matched.len() <= 5);
    }

    #[test]
    fn restart_clears_items() {
        let mut engine = CompletionEngine::new();
        inject_and_settle(&mut engine, vec![make_item("hello")]);
        assert_eq!(engine.total_count(), 1);

        engine.restart();
        settle(&mut engine);
        assert_eq!(engine.total_count(), 0);
    }

    #[test]
    fn push_item_helper() {
        let mut engine = CompletionEngine::new();
        let injector = engine.injector();
        push_item(&injector, make_item("single"));
        settle(&mut engine);
        assert_eq!(engine.total_count(), 1);
    }

    #[test]
    fn push_items_helper() {
        let mut engine = CompletionEngine::new();
        let injector = engine.injector();
        push_items(&injector, vec![make_item("a"), make_item("b")]);
        settle(&mut engine);
        assert_eq!(engine.total_count(), 2);
    }

    #[test]
    fn tick_status_fields() {
        let mut engine = CompletionEngine::new();
        let status = engine.tick(0);
        let _ = status.running;
        let _ = status.changed;
        let debug = format!("{status:?}");
        assert!(debug.contains("TickStatus"));
    }

    #[test]
    fn tick_status_copy() {
        let mut engine = CompletionEngine::new();
        let status = engine.tick(0);
        let copied = status;
        assert_eq!(copied.running, status.running);
        assert_eq!(copied.changed, status.changed);
    }

    #[test]
    fn engine_item_clone() {
        let item = make_item("test");
        let engine_item = EngineItem {
            match_text: Utf32String::from(item.label.as_str()),
            item,
        };
        #[allow(clippy::redundant_clone)]
        let cloned = engine_item.clone();
        assert_eq!(cloned.item.label, "test");
    }

    #[test]
    fn score_ordering() {
        let mut engine = CompletionEngine::new();
        let items = vec![
            make_item("xyzzy"),
            make_item("main_func"),
            make_item("lib_func"),
            make_item("mod_func"),
        ];
        inject_and_settle(&mut engine, items);

        engine.set_pattern("main");
        settle(&mut engine);

        let matched = engine.matched_items(10);
        assert!(!matched.is_empty());
        assert_eq!(matched[0].label, "main_func");
    }

    #[test]
    fn unicode_items() {
        let mut engine = CompletionEngine::new();
        let items = vec![
            make_item("日本語テスト"),
            make_item("中文测试"),
            make_item("한국어"),
        ];
        inject_and_settle(&mut engine, items);
        assert_eq!(engine.total_count(), 3);
        assert_eq!(engine.matched_count(), 3);
    }

    #[test]
    fn case_smart_matching() {
        let mut engine = CompletionEngine::new();
        let items = vec![
            make_item("FooBar"),
            make_item("foobar"),
            make_item("FOOBAR"),
        ];
        inject_and_settle(&mut engine, items);

        engine.set_pattern("foo");
        settle(&mut engine);
        assert_eq!(engine.matched_count(), 3);
    }
}
