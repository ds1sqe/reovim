//! Completion filtering utilities

#![allow(dead_code)] // Filter functions are used by completion engine

use super::item::CompletionItem;

/// Filter and score completion items by prefix
#[must_use]
pub fn filter_by_prefix(items: &[CompletionItem], prefix: &str) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();

    let mut filtered: Vec<_> = items
        .iter()
        .filter(|item| item.filter_text().to_lowercase().starts_with(&prefix_lower))
        .cloned()
        .collect();

    // Sort by priority, then alphabetically
    filtered.sort_by(|a, b| {
        a.sort_priority
            .cmp(&b.sort_priority)
            .then_with(|| a.label.cmp(&b.label))
    });

    filtered
}

/// Filter items that contain the pattern anywhere (case-insensitive)
#[must_use]
pub fn filter_contains(items: &[CompletionItem], pattern: &str) -> Vec<CompletionItem> {
    let pattern_lower = pattern.to_lowercase();

    let mut filtered: Vec<_> = items
        .iter()
        .filter(|item| item.filter_text().to_lowercase().contains(&pattern_lower))
        .cloned()
        .collect();

    // Sort by priority, then by whether it starts with prefix, then alphabetically
    filtered.sort_by(|a, b| {
        let a_starts = a.filter_text().to_lowercase().starts_with(&pattern_lower);
        let b_starts = b.filter_text().to_lowercase().starts_with(&pattern_lower);

        // Items starting with prefix come first
        match (a_starts, b_starts) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a
                .sort_priority
                .cmp(&b.sort_priority)
                .then_with(|| a.label.cmp(&b.label)),
        }
    });

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<CompletionItem> {
        vec![
            CompletionItem::new("apple", "buffer").with_priority(100),
            CompletionItem::new("application", "buffer").with_priority(100),
            CompletionItem::new("Apply", "buffer").with_priority(100),
            CompletionItem::new("banana", "buffer").with_priority(100),
            CompletionItem::new("pineapple", "buffer").with_priority(100),
        ]
    }

    #[test]
    fn test_filter_by_prefix() {
        let items = sample_items();

        let filtered = filter_by_prefix(&items, "app");
        assert_eq!(filtered.len(), 3);
        assert!(
            filtered
                .iter()
                .all(|i| i.label.to_lowercase().starts_with("app"))
        );
    }

    #[test]
    fn test_filter_by_prefix_case_insensitive() {
        let items = sample_items();

        let filtered = filter_by_prefix(&items, "APP");
        assert_eq!(filtered.len(), 3);

        let filtered = filter_by_prefix(&items, "App");
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_filter_by_prefix_empty() {
        let items = sample_items();

        let filtered = filter_by_prefix(&items, "");
        assert_eq!(filtered.len(), 5); // All items match empty prefix
    }

    #[test]
    fn test_filter_by_prefix_no_match() {
        let items = sample_items();

        let filtered = filter_by_prefix(&items, "xyz");
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_contains() {
        let items = sample_items();

        let filtered = filter_contains(&items, "apple");
        assert_eq!(filtered.len(), 2); // "apple" and "pineapple"

        // Items starting with prefix should come first
        assert_eq!(filtered[0].label, "apple");
        assert_eq!(filtered[1].label, "pineapple");
    }

    #[test]
    fn test_filter_sorted_by_priority() {
        let items = vec![
            CompletionItem::new("zebra", "buffer").with_priority(200),
            CompletionItem::new("apple", "buffer").with_priority(50),
            CompletionItem::new("ant", "buffer").with_priority(100),
        ];

        let filtered = filter_by_prefix(&items, "");
        // Sorted by priority first
        assert_eq!(filtered[0].label, "apple"); // priority 50
        assert_eq!(filtered[1].label, "ant"); // priority 100
        assert_eq!(filtered[2].label, "zebra"); // priority 200
    }
}
