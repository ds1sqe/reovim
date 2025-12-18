//! Autocompletion module
//!
//! This module provides an extensible autocompletion system with:
//! - Trait-based completion sources for extensibility
//! - Prefix and fuzzy matching support
//! - Debounced trigger detection
//! - State management for popup UI

pub mod filter;
pub mod item;
pub mod source;
pub mod state;
pub mod trigger;

pub use {
    item::CompletionItem,
    source::{CompletionContext, CompletionSource},
    state::CompletionState,
    trigger::{TriggerConfig, TriggerDetector, TriggerResult},
};

use std::sync::Arc;

use crate::buffer::Line;

use self::source::buffer::BufferWordsSource;

/// Main completion engine that coordinates sources and state
pub struct CompletionEngine {
    /// Registered completion sources
    sources: Vec<Arc<dyn CompletionSource>>,
    /// Trigger configuration
    trigger_config: TriggerConfig,
    /// Maximum items to show
    max_items: usize,
}

impl CompletionEngine {
    /// Create a new completion engine
    #[must_use]
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            trigger_config: TriggerConfig::default(),
            max_items: 10,
        }
    }

    /// Register a completion source
    pub fn register_source(&mut self, source: Arc<dyn CompletionSource>) {
        self.sources.push(source);
        // Keep sources sorted by priority
        self.sources.sort_by_key(|s| s.priority());
    }

    /// Get all trigger characters from registered sources
    #[must_use]
    pub fn trigger_characters(&self) -> Vec<char> {
        let mut chars = self.trigger_config.trigger_chars.clone();
        for source in &self.sources {
            if let Some(source_chars) = source.trigger_characters() {
                chars.extend(source_chars);
            }
        }
        chars.sort_unstable();
        chars.dedup();
        chars
    }

    /// Fetch completions from all sources
    pub async fn complete(
        &self,
        ctx: &CompletionContext,
        buffer_content: &[Line],
    ) -> Vec<CompletionItem> {
        use futures::future::join_all;

        // Collect futures from available sources
        let futures: Vec<_> = self
            .sources
            .iter()
            .filter(|s| s.is_available(ctx))
            .map(|source| source.complete(ctx, buffer_content))
            .collect();

        // Execute all sources concurrently
        let results = join_all(futures).await;

        // Merge results
        let mut all_items: Vec<CompletionItem> = results.into_iter().flatten().collect();

        // Filter by prefix
        all_items = filter::filter_by_prefix(&all_items, &ctx.prefix);

        // Limit results
        all_items.truncate(self.max_items);

        all_items
    }

    /// Get the trigger configuration
    #[must_use]
    pub const fn trigger_config(&self) -> &TriggerConfig {
        &self.trigger_config
    }

    /// Set maximum items to return
    pub const fn set_max_items(&mut self, max: usize) {
        self.max_items = max;
    }
}

impl Default for CompletionEngine {
    fn default() -> Self {
        let mut engine = Self::new();
        // Register default buffer words source
        engine.register_source(Arc::new(BufferWordsSource::new()));
        engine
    }
}
