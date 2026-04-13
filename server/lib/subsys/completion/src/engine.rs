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
#[path = "engine_tests.rs"]
mod tests;
