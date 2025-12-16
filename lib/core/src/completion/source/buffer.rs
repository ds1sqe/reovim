//! Buffer words completion source
//!
//! Provides completion candidates from words in the current buffer.

use std::{collections::HashSet, future::Future, pin::Pin};

use crate::buffer::Line;

use super::{CompletionContext, CompletionSource};
use crate::completion::item::CompletionItem;

/// Completion source that provides words from the current buffer
pub struct BufferWordsSource {
    /// Minimum word length to include
    min_word_length: usize,
}

impl BufferWordsSource {
    /// Create a new buffer words source
    #[must_use]
    pub const fn new() -> Self {
        Self { min_word_length: 2 }
    }

    /// Set minimum word length
    #[must_use]
    pub const fn with_min_word_length(mut self, len: usize) -> Self {
        self.min_word_length = len;
        self
    }

    /// Check if a character is a word character
    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    /// Extract words from buffer content, excluding the word at current position
    fn extract_words(
        &self,
        content: &[Line],
        current_pos_y: u16,
        word_start_col: u16,
        current_col: u16,
    ) -> Vec<String> {
        let mut words = HashSet::new();

        for (line_idx, line) in content.iter().enumerate() {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut i = 0;

            while i < chars.len() {
                // Skip non-word characters
                if !Self::is_word_char(chars[i]) {
                    i += 1;
                    continue;
                }

                // Found start of a word
                let word_start = i;
                while i < chars.len() && Self::is_word_char(chars[i]) {
                    i += 1;
                }
                let word_end = i;

                // Check if this is the word being typed (skip it)
                // Only exclude if we're actually typing something (word_start_col < current_col)
                let is_current_word = word_start_col < current_col
                    && line_idx == current_pos_y as usize
                    && word_start <= word_start_col as usize
                    && word_end >= current_col as usize;

                if !is_current_word {
                    let word: String = chars[word_start..word_end].iter().collect();
                    if word.len() >= self.min_word_length {
                        words.insert(word);
                    }
                }
            }
        }

        let mut result: Vec<_> = words.into_iter().collect();
        result.sort();
        result
    }
}

impl Default for BufferWordsSource {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionSource for BufferWordsSource {
    fn name(&self) -> &'static str {
        "buffer_words"
    }

    fn priority(&self) -> u32 {
        50 // Higher priority than external sources
    }

    fn complete<'a>(
        &'a self,
        ctx: &'a CompletionContext,
        buffer_content: &'a [Line],
    ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
        Box::pin(async move {
            let words = self.extract_words(
                buffer_content,
                ctx.position.y,
                ctx.word_start_col,
                ctx.position.x,
            );

            words
                .into_iter()
                .map(|word| CompletionItem::new(word, self.name()))
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Position;

    fn make_lines(text: &str) -> Vec<Line> {
        text.lines().map(Line::from).collect()
    }

    fn make_context(prefix: &str, pos_y: u16, pos_x: u16, word_start: u16) -> CompletionContext {
        CompletionContext {
            buffer_id: 0,
            position: Position { x: pos_x, y: pos_y },
            line: String::new(),
            word_start_col: word_start,
            prefix: prefix.to_string(),
            trigger_char: None,
        }
    }

    #[tokio::test]
    async fn test_extract_words_basic() {
        let source = BufferWordsSource::new();
        let lines = make_lines("hello world foo bar");
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, &lines).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
        assert!(labels.contains(&"foo"));
        assert!(labels.contains(&"bar"));
    }

    #[tokio::test]
    async fn test_extract_words_multiline() {
        let source = BufferWordsSource::new();
        let lines = make_lines("first line\nsecond line\nthird line");
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, &lines).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"first"));
        assert!(labels.contains(&"second"));
        assert!(labels.contains(&"third"));
        assert!(labels.contains(&"line"));
    }

    #[tokio::test]
    async fn test_excludes_current_word() {
        let source = BufferWordsSource::new();
        let lines = make_lines("hello hel world");
        // Cursor is at position after "hel" (the word being typed)
        let ctx = make_context("hel", 0, 9, 6);

        let items = source.complete(&ctx, &lines).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
        // Should not contain "hel" as that's the word being typed
    }

    #[tokio::test]
    async fn test_min_word_length() {
        let source = BufferWordsSource::new().with_min_word_length(4);
        let lines = make_lines("a ab abc abcd abcde");
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, &lines).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(!labels.contains(&"a"));
        assert!(!labels.contains(&"ab"));
        assert!(!labels.contains(&"abc"));
        assert!(labels.contains(&"abcd"));
        assert!(labels.contains(&"abcde"));
    }

    #[tokio::test]
    async fn test_deduplicates_words() {
        let source = BufferWordsSource::new();
        let lines = make_lines("hello hello hello world world");
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, &lines).await;

        // Should only have unique words
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_handles_underscores() {
        let source = BufferWordsSource::new();
        let lines = make_lines("snake_case camelCase _private __dunder__");
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, &lines).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"snake_case"));
        assert!(labels.contains(&"camelCase"));
        assert!(labels.contains(&"_private"));
        assert!(labels.contains(&"__dunder__"));
    }

    #[tokio::test]
    async fn test_handles_numbers() {
        let source = BufferWordsSource::new();
        let lines = make_lines("var1 var2 123 abc123");
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, &lines).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"var1"));
        assert!(labels.contains(&"var2"));
        assert!(labels.contains(&"123"));
        assert!(labels.contains(&"abc123"));
    }
}
