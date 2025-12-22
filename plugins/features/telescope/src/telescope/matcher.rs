//! Nucleo-based fuzzy matching

use nucleo::{
    Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};

use super::item::TelescopeItem;

/// Wrapper around nucleo matcher for fuzzy matching telescope items
pub struct TelescopeMatcher {
    matcher: Matcher,
    pattern: Pattern,
}

impl TelescopeMatcher {
    /// Create a new matcher
    #[must_use]
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(nucleo::Config::DEFAULT),
            pattern: Pattern::parse("", CaseMatching::Smart, Normalization::Smart),
        }
    }

    /// Update the search pattern
    pub fn set_pattern(&mut self, query: &str) {
        self.pattern
            .reparse(query, CaseMatching::Smart, Normalization::Smart);
    }

    /// Check if pattern is empty
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty is not const-stable
    fn is_pattern_empty(&self) -> bool {
        self.pattern.atoms.is_empty()
    }

    /// Match items against the current pattern and return sorted results
    #[must_use]
    pub fn match_items(&mut self, items: Vec<TelescopeItem>) -> Vec<TelescopeItem> {
        if self.is_pattern_empty() {
            return items;
        }

        let mut scored: Vec<(TelescopeItem, u32)> = items
            .into_iter()
            .filter_map(|item| {
                let mut buf = Vec::new();
                let haystack = Utf32Str::new(item.match_text(), &mut buf);
                self.pattern
                    .score(haystack, &mut self.matcher)
                    .map(|score| (item.with_score(score), score))
            })
            .collect();

        // Sort by score (highest first)
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        scored.into_iter().map(|(item, _)| item).collect()
    }

    /// Check if an item matches the current pattern
    #[must_use]
    pub fn matches(&mut self, text: &str) -> bool {
        if self.is_pattern_empty() {
            return true;
        }
        let mut buf = Vec::new();
        let haystack = Utf32Str::new(text, &mut buf);
        self.pattern.score(haystack, &mut self.matcher).is_some()
    }

    /// Get score for a single item
    #[must_use]
    pub fn score(&mut self, text: &str) -> Option<u32> {
        if self.is_pattern_empty() {
            return Some(0);
        }
        let mut buf = Vec::new();
        let haystack = Utf32Str::new(text, &mut buf);
        self.pattern.score(haystack, &mut self.matcher)
    }
}

impl Default for TelescopeMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::telescope::item::TelescopeData, std::path::PathBuf};

    fn make_item(display: &str) -> TelescopeItem {
        TelescopeItem::new(
            display,
            display,
            TelescopeData::FilePath(PathBuf::from(display)),
            "test",
        )
    }

    #[test]
    fn test_empty_pattern() {
        let mut matcher = TelescopeMatcher::new();
        let items = vec![make_item("foo.rs"), make_item("bar.rs")];
        let result = matcher.match_items(items);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_fuzzy_match() {
        let mut matcher = TelescopeMatcher::new();
        matcher.set_pattern("fr");

        let items = vec![
            make_item("foo.rs"),
            make_item("bar.rs"),
            make_item("foobar.rs"),
        ];

        let result = matcher.match_items(items);

        // "foo.rs" and "foobar.rs" should match "fr" (f...r...s)
        assert!(result.iter().any(|i| i.display == "foo.rs"));
        // "bar.rs" should not match
        assert!(!result.iter().any(|i| i.display == "bar.rs"));
    }

    #[test]
    fn test_score_ordering() {
        let mut matcher = TelescopeMatcher::new();
        matcher.set_pattern("main");

        let items = vec![
            make_item("src/lib/main.rs"),
            make_item("main.rs"),
            make_item("src/main_helper.rs"),
        ];

        let result = matcher.match_items(items);

        // "main.rs" should be first (exact match)
        assert!(result.first().map(|i| i.display.as_str()) == Some("main.rs"));
    }

    #[test]
    fn test_matches() {
        let mut matcher = TelescopeMatcher::new();
        matcher.set_pattern("rs");

        assert!(matcher.matches("test.rs"));
        assert!(!matcher.matches("test.py"));
    }
}
