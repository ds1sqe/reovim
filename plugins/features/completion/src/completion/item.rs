//! Completion item types

/// Represents a single completion candidate
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// The text to insert on completion
    pub insert_text: String,
    /// Display label (may differ from `insert_text`)
    pub label: String,
    /// Optional detail/type info (e.g., "fn", "var", "keyword")
    pub detail: Option<String>,
    /// Source identifier for debugging/prioritization
    pub source: &'static str,
    /// Sort priority (lower = higher priority)
    pub sort_priority: u32,
    /// Filter text (for matching, defaults to label if None)
    pub filter_text: Option<String>,
}

impl CompletionItem {
    /// Create a new completion item with just insert text and source
    #[must_use]
    pub fn new(insert_text: impl Into<String>, source: &'static str) -> Self {
        let text = insert_text.into();
        Self {
            label: text.clone(),
            insert_text: text,
            detail: None,
            source,
            sort_priority: 100,
            filter_text: None,
        }
    }

    /// Create a completion item with a custom label
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the detail text
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set the sort priority
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.sort_priority = priority;
        self
    }

    /// Set custom filter text
    #[must_use]
    pub fn with_filter_text(mut self, filter_text: impl Into<String>) -> Self {
        self.filter_text = Some(filter_text.into());
        self
    }

    /// Get text used for filtering/matching
    #[must_use]
    pub fn filter_text(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_completion_item() {
        let item = CompletionItem::new("hello", "buffer");
        assert_eq!(item.insert_text, "hello");
        assert_eq!(item.label, "hello");
        assert_eq!(item.source, "buffer");
        assert_eq!(item.sort_priority, 100);
        assert!(item.detail.is_none());
        assert!(item.filter_text.is_none());
    }

    #[test]
    fn test_builder_methods() {
        let item = CompletionItem::new("println!", "snippet")
            .with_label("println!()")
            .with_detail("macro")
            .with_priority(50)
            .with_filter_text("println");

        assert_eq!(item.insert_text, "println!");
        assert_eq!(item.label, "println!()");
        assert_eq!(item.detail.as_deref(), Some("macro"));
        assert_eq!(item.sort_priority, 50);
        assert_eq!(item.filter_text(), "println");
    }

    #[test]
    fn test_filter_text_fallback() {
        let item = CompletionItem::new("test", "buffer");
        assert_eq!(item.filter_text(), "test"); // Falls back to label
    }
}
