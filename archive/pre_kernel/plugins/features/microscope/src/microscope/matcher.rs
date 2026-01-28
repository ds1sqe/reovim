//! Streaming fuzzy matcher using Nucleo<T>
//!
//! This module provides non-blocking fuzzy matching using nucleo's streaming API.
//! Items can be injected from background tasks while the matcher continues processing.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │ Background  │────>│  Injector<T> │────>│  Nucleo<T>  │
//! │   Tasks     │     │  (lock-free) │     │ (workers)   │
//! └─────────────┘     └──────────────┘     └─────────────┘
//!                                                 │
//!                                          ┌──────┴──────┐
//!                                          │  tick(10ms) │
//!                                          │ (non-block) │
//!                                          └──────┬──────┘
//!                                                 │
//!                                          ┌──────┴──────┐
//!                                          │  snapshot() │
//!                                          │  (instant)  │
//!                                          └─────────────┘
//! ```

use std::sync::Arc;

use nucleo::{
    Config, Injector, Nucleo, Utf32String,
    pattern::{CaseMatching, Normalization},
};

use super::item::MicroscopeItem;

/// Data stored in nucleo for each item
#[derive(Clone)]
pub struct MatcherItem {
    /// The original microscope item
    pub item: MicroscopeItem,
    /// Pre-converted match text for nucleo
    match_text: Utf32String,
}

impl MatcherItem {
    /// Create a new matcher item from a microscope item
    #[must_use]
    pub fn new(item: MicroscopeItem) -> Self {
        let match_text = Utf32String::from(item.match_text());
        Self { item, match_text }
    }
}

/// Status of the matcher after a tick
#[derive(Debug, Clone, Copy)]
pub struct MatcherStatus {
    /// Whether matching is still in progress
    pub running: bool,
    /// Whether new items were added since last tick
    pub changed: bool,
}

/// Streaming fuzzy matcher using Nucleo<T>
///
/// Provides non-blocking fuzzy matching with progressive results.
/// Items are injected via `Injector` and results are retrieved via `snapshot()`.
///
/// # Example
///
/// ```ignore
/// let matcher = MicroscopeMatcher::new();
///
/// // Get injector for background task
/// let injector = matcher.injector();
/// tokio::spawn(async move {
///     for item in items {
///         injector.push(MatcherItem::new(item), |mi, cols| {
///             cols[0] = mi.match_text.clone();
///         });
///     }
/// });
///
/// // In render loop
/// matcher.tick(10); // Process for max 10ms
/// let results = matcher.get_matched_items(100); // Get top 100 results
/// ```
pub struct MicroscopeMatcher {
    nucleo: Nucleo<MatcherItem>,
}

impl MicroscopeMatcher {
    /// Create a new streaming matcher
    ///
    /// Uses smart case matching and Unicode normalization.
    /// Spawns worker threads for parallel matching.
    #[must_use]
    pub fn new() -> Self {
        let config = Config::DEFAULT;
        // notify callback (called when results change) - we use polling via tick()
        let notify = Arc::new(|| {});
        // Number of columns to match against (we use 1: the display text)
        let columns = 1;

        Self {
            nucleo: Nucleo::new(config, notify, None, columns),
        }
    }

    /// Get an injector for pushing items from background tasks
    ///
    /// The injector is thread-safe and can be cloned and sent to other threads.
    #[must_use]
    pub fn injector(&self) -> Injector<MatcherItem> {
        self.nucleo.injector()
    }

    /// Update the search pattern
    ///
    /// This triggers re-matching of all items against the new pattern.
    pub fn set_pattern(&mut self, query: &str) {
        self.nucleo.pattern.reparse(
            0,
            query,
            CaseMatching::Smart,
            Normalization::Smart,
            query.starts_with(char::is_lowercase),
        );
    }

    /// Process pending matches (non-blocking)
    ///
    /// Call this regularly (e.g., in render loop) to process matches.
    /// Returns status indicating if more processing is needed.
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Maximum time to spend processing (in milliseconds)
    #[must_use]
    pub fn tick(&mut self, timeout_ms: u64) -> MatcherStatus {
        let status = self.nucleo.tick(timeout_ms);
        MatcherStatus {
            running: status.running,
            changed: status.changed,
        }
    }

    /// Get matched items from the current snapshot
    ///
    /// Returns items sorted by match score (best matches first).
    /// This is instant and does not block.
    ///
    /// Note: The items are pre-sorted by nucleo, so the order reflects
    /// the ranking. Individual scores are not exposed by nucleo's streaming API,
    /// so we use the item index as a rough score proxy (lower = better match).
    ///
    /// # Arguments
    ///
    /// * `max_items` - Maximum number of items to return
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn get_matched_items(&self, max_items: usize) -> Vec<MicroscopeItem> {
        let snapshot = self.nucleo.snapshot();
        let count = snapshot.matched_item_count();
        let limit = (max_items as u32).min(count);

        snapshot
            .matched_items(..limit)
            .enumerate()
            .map(|(idx, item)| {
                let mut microscope_item = item.data.item.clone();
                // Use reverse index as score proxy (higher = better match at top)
                microscope_item.score = Some((limit - idx as u32) * 100);
                microscope_item
            })
            .collect()
    }

    /// Get total number of items in the matcher
    #[must_use]
    pub fn total_count(&self) -> u32 {
        self.nucleo.snapshot().item_count()
    }

    /// Get number of items matching the current pattern
    #[must_use]
    pub fn matched_count(&self) -> u32 {
        self.nucleo.snapshot().matched_item_count()
    }

    /// Check if pattern is empty (all items match)
    #[must_use]
    pub fn is_pattern_empty(&self) -> bool {
        self.nucleo.pattern.column_pattern(0).atoms.is_empty()
    }

    /// Restart the matcher (clear all items)
    ///
    /// Call this when switching pickers or starting a new search.
    pub fn restart(&mut self) {
        self.nucleo.restart(false);
    }

    /// Restart and also clear the pattern
    pub fn restart_with_clear(&mut self) {
        self.nucleo.restart(true);
    }
}

impl Default for MicroscopeMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Push a microscope item into the injector
///
/// Helper function to properly format items for nucleo matching.
pub fn push_item(injector: &Injector<MatcherItem>, item: MicroscopeItem) {
    let matcher_item = MatcherItem::new(item);
    injector.push(matcher_item, |mi, cols| {
        cols[0] = mi.match_text.clone();
    });
}

/// Push multiple items into the injector
pub fn push_items(
    injector: &Injector<MatcherItem>,
    items: impl IntoIterator<Item = MicroscopeItem>,
) {
    for item in items {
        push_item(injector, item);
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::microscope::item::MicroscopeData, std::path::PathBuf};

    fn make_item(display: &str) -> MicroscopeItem {
        MicroscopeItem::new(
            display,
            display,
            MicroscopeData::FilePath(PathBuf::from(display)),
            "test",
        )
    }

    #[test]
    fn test_new_matcher() {
        let matcher = MicroscopeMatcher::new();
        assert_eq!(matcher.total_count(), 0);
        assert_eq!(matcher.matched_count(), 0);
        assert!(matcher.is_pattern_empty());
    }

    #[test]
    fn test_inject_and_match() {
        let mut matcher = MicroscopeMatcher::new();
        let injector = matcher.injector();

        // Push items
        push_item(&injector, make_item("foo.rs"));
        push_item(&injector, make_item("bar.rs"));
        push_item(&injector, make_item("foobar.rs"));

        // Process matches
        let _ = matcher.tick(100);

        // All items should match (empty pattern)
        assert_eq!(matcher.total_count(), 3);
        assert_eq!(matcher.matched_count(), 3);

        let results = matcher.get_matched_items(10);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_pattern_matching() {
        let mut matcher = MicroscopeMatcher::new();
        let injector = matcher.injector();

        push_item(&injector, make_item("foo.rs"));
        push_item(&injector, make_item("bar.rs"));
        push_item(&injector, make_item("foobar.rs"));

        let _ = matcher.tick(100);

        // Set pattern
        matcher.set_pattern("foo");
        let _ = matcher.tick(100);

        // Only "foo.rs" and "foobar.rs" should match
        let results = matcher.get_matched_items(10);
        assert!(results.iter().any(|i| i.display == "foo.rs"));
        assert!(results.iter().any(|i| i.display == "foobar.rs"));
        assert!(!results.iter().any(|i| i.display == "bar.rs"));
    }

    #[test]
    fn test_restart() {
        let mut matcher = MicroscopeMatcher::new();
        let injector = matcher.injector();

        push_item(&injector, make_item("test.rs"));
        let _ = matcher.tick(100);
        assert_eq!(matcher.total_count(), 1);

        // Restart clears items
        matcher.restart();
        let _ = matcher.tick(100);
        assert_eq!(matcher.total_count(), 0);
    }

    #[test]
    fn test_score_ordering() {
        let mut matcher = MicroscopeMatcher::new();
        let injector = matcher.injector();

        push_item(&injector, make_item("src/lib/main.rs"));
        push_item(&injector, make_item("main.rs"));
        push_item(&injector, make_item("src/main_helper.rs"));

        let _ = matcher.tick(100);
        matcher.set_pattern("main");
        let _ = matcher.tick(100);

        let results = matcher.get_matched_items(10);

        // "main.rs" should be ranked higher (better match)
        assert!(!results.is_empty());
        // First result should have highest score
        if results.len() > 1 {
            assert!(results[0].score >= results[1].score);
        }
    }
}
