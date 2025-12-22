//! Treesitter plugin events
//!
//! Events for communication between treesitter plugin and other components.

use std::sync::Arc;

use reovim_core::{event_bus::Event, folding::FoldRange, highlight::Highlight};

use crate::registry::LanguageSupport;

/// Event emitted by language plugins to register themselves
#[derive(Clone)]
pub struct RegisterLanguage {
    /// The language support implementation
    pub language: Arc<dyn LanguageSupport>,
}

impl std::fmt::Debug for RegisterLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterLanguage")
            .field("language_id", &self.language.language_id())
            .finish()
    }
}

impl Event for RegisterLanguage {}

/// Event emitted when highlights are ready for a buffer
#[derive(Debug, Clone)]
pub struct HighlightsReady {
    /// Buffer ID
    pub buffer_id: usize,
    /// Generated highlights
    pub highlights: Vec<Highlight>,
}

impl Event for HighlightsReady {}

/// Event emitted when fold ranges are computed
///
/// Note: This is a wrapper around the core FoldRangesComputed event
/// for internal use within the treesitter plugin.
#[derive(Debug, Clone)]
pub struct TreesitterFoldRanges {
    /// Buffer ID
    pub buffer_id: usize,
    /// Computed fold ranges
    pub ranges: Vec<FoldRange>,
}

impl Event for TreesitterFoldRanges {}

/// Event to request a buffer parse
#[derive(Debug, Clone)]
pub struct ParseRequest {
    /// Buffer ID to parse
    pub buffer_id: usize,
    /// Whether to do incremental parse (if edit info available)
    pub incremental: bool,
}

impl Event for ParseRequest {}

/// Event emitted when a parse completes
#[derive(Debug, Clone)]
pub struct ParseCompleted {
    /// Buffer ID
    pub buffer_id: usize,
    /// Language ID of the parsed buffer
    pub language_id: String,
}

impl Event for ParseCompleted {}
