//! Buffer words completion source.
//!
//! Scans the current buffer content for word tokens and provides them
//! as completion items. Words must be at least 2 characters long.
//! The word at the current cursor position is excluded from results.

use reovim_driver_completion::{
    CompletionContext, CompletionItem, CompletionKind, CompletionSource,
};

/// Minimum word length to include in completions.
const MIN_WORD_LEN: usize = 2;

/// Completion source that extracts words from the buffer content.
///
/// Provides low-priority completions (priority 100) from all words in
/// the current buffer. Deduplicates results and excludes the word
/// under the cursor.
#[derive(Debug)]
pub struct BufferWordsSource;

impl CompletionSource for BufferWordsSource {
    fn id(&self) -> &'static str {
        "buffer"
    }

    fn priority(&self) -> u16 {
        100
    }

    fn is_available(&self, _ctx: &CompletionContext) -> bool {
        true
    }

    fn complete(&self, ctx: &CompletionContext) -> Vec<CompletionItem> {
        let prefix = &ctx.prefix;
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();

        for word in extract_words(&ctx.content) {
            if word.len() < MIN_WORD_LEN {
                continue;
            }
            // Skip the exact prefix (word under cursor).
            if word == prefix {
                continue;
            }
            // Case-insensitive prefix match.
            if !word
                .to_ascii_lowercase()
                .starts_with(&prefix.to_ascii_lowercase())
            {
                continue;
            }
            if !seen.insert(word.to_owned()) {
                continue;
            }
            items.push(CompletionItem {
                label: word.to_owned(),
                insert_text: word.to_owned(),
                kind: CompletionKind::Text,
                detail: None,
                documentation: None,
                source_id: "buffer",
                is_snippet: false,
                sort_priority: 100,
            });
        }

        items
    }
}

/// Extract word tokens from content.
///
/// A "word" is a contiguous sequence of alphanumeric or underscore characters.
fn extract_words(content: &str) -> impl Iterator<Item = &str> {
    content
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
}

#[cfg(test)]
#[path = "buffer_words_tests.rs"]
mod tests;
