//! Completion source trait and context

pub mod buffer;

use std::{future::Future, pin::Pin};

use reovim_core::{buffer::Line, screen::Position};

use super::item::CompletionItem;

/// Context provided to completion sources
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// Buffer ID being completed
    pub buffer_id: usize,
    /// Current cursor position
    pub position: Position,
    /// Current line text
    pub line: String,
    /// Column where the word being completed starts
    pub word_start_col: u16,
    /// The partial word typed so far (prefix)
    pub prefix: String,
    /// Trigger character if completion was triggered by one
    pub trigger_char: Option<char>,
}

impl CompletionContext {
    /// Create a new completion context
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String::new() is not const
    pub fn new(
        buffer_id: usize,
        position: Position,
        line: String,
        word_start_col: u16,
        prefix: String,
    ) -> Self {
        Self {
            buffer_id,
            position,
            line,
            word_start_col,
            prefix,
            trigger_char: None,
        }
    }

    /// Set the trigger character
    #[must_use]
    pub const fn with_trigger_char(mut self, ch: char) -> Self {
        self.trigger_char = Some(ch);
        self
    }
}

/// Async completion source trait
///
/// Implement this trait to create custom completion sources.
/// Sources are called concurrently to provide completion items.
pub trait CompletionSource: Send + Sync {
    /// Unique identifier for this source
    fn name(&self) -> &'static str;

    /// Priority for merging results (lower = higher priority)
    fn priority(&self) -> u32 {
        100
    }

    /// Whether this source should be active in the given context
    fn is_available(&self, _ctx: &CompletionContext) -> bool {
        true
    }

    /// Fetch completions asynchronously
    ///
    /// Returns a boxed future for object safety
    fn complete<'a>(
        &'a self,
        ctx: &'a CompletionContext,
        buffer_content: &'a [Line],
    ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>>;

    /// Optional: trigger characters that should start completion immediately
    fn trigger_characters(&self) -> Option<&[char]> {
        None
    }
}
