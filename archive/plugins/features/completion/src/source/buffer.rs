//! Buffer words completion source
//!
//! Provides completion candidates from words in the current buffer.
//! Updated to implement the new SourceSupport trait.

#![allow(dead_code)] // Methods are part of public API

use std::{collections::HashSet, future::Future, pin::Pin};

use reovim_core::completion::{CompletionContext, CompletionItem};

use crate::registry::SourceSupport;

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
        content: &str,
        current_row: u32,
        word_start_col: u32,
        current_col: u32,
    ) -> Vec<String> {
        let mut words = HashSet::new();

        for (line_idx, line) in content.lines().enumerate() {
            let chars: Vec<char> = line.chars().collect();
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
                    && line_idx == current_row as usize
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

impl SourceSupport for BufferWordsSource {
    fn source_id(&self) -> &'static str {
        "buffer_words"
    }

    fn priority(&self) -> u32 {
        100 // Default priority for buffer source
    }

    fn complete<'a>(
        &'a self,
        ctx: &'a CompletionContext,
        content: &'a str,
    ) -> Pin<Box<dyn Future<Output = Vec<CompletionItem>> + Send + 'a>> {
        Box::pin(async move {
            let words =
                self.extract_words(content, ctx.cursor_row, ctx.word_start_col, ctx.cursor_col);

            words
                .into_iter()
                .map(|word| CompletionItem::new(word, self.source_id()))
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(prefix: &str, row: u32, col: u32, word_start: u32) -> CompletionContext {
        CompletionContext::new(0, row, col, String::new(), prefix.to_string(), word_start)
    }

    #[tokio::test]
    async fn test_extract_words_basic() {
        let source = BufferWordsSource::new();
        let content = "hello world foo bar";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
        assert!(labels.contains(&"foo"));
        assert!(labels.contains(&"bar"));
    }

    #[tokio::test]
    async fn test_extract_words_multiline() {
        let source = BufferWordsSource::new();
        let content = "first line\nsecond line\nthird line";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"first"));
        assert!(labels.contains(&"second"));
        assert!(labels.contains(&"third"));
        assert!(labels.contains(&"line"));
    }

    #[tokio::test]
    async fn test_excludes_current_word() {
        let source = BufferWordsSource::new();
        let content = "hello hel world";
        // Cursor is at position after "hel" (the word being typed)
        let ctx = make_context("hel", 0, 9, 6);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
        // Should not contain "hel" as that's the word being typed
    }

    #[tokio::test]
    async fn test_min_word_length() {
        let source = BufferWordsSource::new().with_min_word_length(4);
        let content = "a ab abc abcd abcde";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
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
        let content = "hello hello hello world world";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;

        // Should only have unique words
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_handles_underscores() {
        let source = BufferWordsSource::new();
        let content = "snake_case camelCase _private __dunder__";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"snake_case"));
        assert!(labels.contains(&"camelCase"));
        assert!(labels.contains(&"_private"));
        assert!(labels.contains(&"__dunder__"));
    }

    #[tokio::test]
    async fn test_handles_numbers() {
        let source = BufferWordsSource::new();
        let content = "var1 var2 123 abc123";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"var1"));
        assert!(labels.contains(&"var2"));
        assert!(labels.contains(&"123"));
        assert!(labels.contains(&"abc123"));
    }

    #[tokio::test]
    async fn test_empty_buffer() {
        let source = BufferWordsSource::new();
        let content = "";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_whitespace_only_buffer() {
        let source = BufferWordsSource::new();
        let content = "   \n\t\n   ";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_unicode_words() {
        let source = BufferWordsSource::new();
        let content = "日本語 hello 中文 world こんにちは";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"日本語"));
        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"中文"));
        assert!(labels.contains(&"world"));
        assert!(labels.contains(&"こんにちは"));
    }

    #[tokio::test]
    async fn test_unicode_mixed_with_ascii() {
        let source = BufferWordsSource::new();
        let content = "变量名1 variable2 函数名_test";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"变量名1"));
        assert!(labels.contains(&"variable2"));
        assert!(labels.contains(&"函数名_test"));
    }

    #[tokio::test]
    async fn test_emoji_handling() {
        let source = BufferWordsSource::new();
        // Emojis should break word boundaries
        let content = "hello🎉world test";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        // "hello" and "world" should be separate words (emoji breaks them)
        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
        assert!(labels.contains(&"test"));
    }

    #[tokio::test]
    async fn test_very_long_line() {
        let source = BufferWordsSource::new();
        // Create a very long line with many words
        let words: Vec<String> = (0..1000).map(|i| format!("word{}", i)).collect();
        let content = words.join(" ");
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content.as_str()).await;

        // Should contain all 1000 unique words
        assert_eq!(items.len(), 1000);
    }

    #[tokio::test]
    async fn test_special_characters_as_word_boundaries() {
        let source = BufferWordsSource::new();
        let content = "foo.bar baz::qux hello->world test[0]";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        // Each should be split at special characters
        assert!(labels.contains(&"foo"));
        assert!(labels.contains(&"bar"));
        assert!(labels.contains(&"baz"));
        assert!(labels.contains(&"qux"));
        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
        assert!(labels.contains(&"test"));
    }

    #[tokio::test]
    async fn test_single_char_words_filtered() {
        let source = BufferWordsSource::new(); // min_word_length = 2
        let content = "a b c ab abc";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        // Single char words should be filtered out (min_word_length = 2)
        assert!(!labels.contains(&"a"));
        assert!(!labels.contains(&"b"));
        assert!(!labels.contains(&"c"));
        assert!(labels.contains(&"ab"));
        assert!(labels.contains(&"abc"));
    }

    #[tokio::test]
    async fn test_cursor_at_beginning_of_word() {
        let source = BufferWordsSource::new();
        let content = "hello world";
        // Cursor at column 0, word_start at 0, cursor_col at 0 (no prefix typed)
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        // Both words should be included
        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
    }

    #[tokio::test]
    async fn test_cursor_in_middle_of_word() {
        let source = BufferWordsSource::new();
        let content = "hello world testing";
        // Cursor is in the middle of "world" at row 0, col 8, word_start 6
        let ctx = make_context("wo", 0, 8, 6);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        // "world" should not be included (it's the word being typed)
        assert!(!labels.contains(&"world"));
        assert!(labels.contains(&"hello"));
        assert!(labels.contains(&"testing"));
    }

    #[tokio::test]
    async fn test_multiline_with_empty_lines() {
        let source = BufferWordsSource::new();
        let content = "first\n\nsecond\n\n\nthird";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"first"));
        assert!(labels.contains(&"second"));
        assert!(labels.contains(&"third"));
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn test_tabs_as_word_boundaries() {
        let source = BufferWordsSource::new();
        let content = "foo\tbar\tbaz";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"foo"));
        assert!(labels.contains(&"bar"));
        assert!(labels.contains(&"baz"));
    }

    #[tokio::test]
    async fn test_words_with_trailing_numbers() {
        let source = BufferWordsSource::new();
        let content = "test1 test2 test3 test123";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();

        assert!(labels.contains(&"test1"));
        assert!(labels.contains(&"test2"));
        assert!(labels.contains(&"test3"));
        assert!(labels.contains(&"test123"));
        assert_eq!(items.len(), 4);
    }

    #[tokio::test]
    async fn test_source_id() {
        let source = BufferWordsSource::new();
        assert_eq!(source.source_id(), "buffer_words");
    }

    #[tokio::test]
    async fn test_priority() {
        let source = BufferWordsSource::new();
        assert_eq!(source.priority(), 100);
    }

    #[tokio::test]
    async fn test_completion_item_has_correct_source() {
        let source = BufferWordsSource::new();
        let content = "hello world";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;

        for item in &items {
            assert_eq!(item.source, "buffer_words");
        }
    }

    #[tokio::test]
    async fn test_same_word_different_lines_deduplicated() {
        let source = BufferWordsSource::new();
        let content = "function\nfunction\nfunction";
        let ctx = make_context("", 0, 0, 0);

        let items = source.complete(&ctx, content).await;

        // Should only have one "function" item
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "function");
    }
}
