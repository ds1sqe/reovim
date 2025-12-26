//! Abstract completion provider trait for auto-completion
//!
//! Core defines the traits, plugins provide implementations.
//! This allows different completion backends (buffer words, LSP, snippets, AI, etc.)

use std::sync::Arc;

/// Completion item kind (subset of LSP `CompletionItemKind`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompletionKind {
    #[default]
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Unit,
    Value,
    Enum,
    Keyword,
    Snippet,
    Color,
    File,
    Reference,
    Folder,
    EnumMember,
    Constant,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

/// A completion item returned by a completion provider
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Text to insert when this item is selected
    pub insert_text: String,
    /// Display label shown in the completion menu
    pub label: String,
    /// Optional detail text (e.g., type info, signature)
    pub detail: Option<String>,
    /// Optional documentation for preview
    pub documentation: Option<String>,
    /// Source identifier (e.g., "buffer", "lsp", "path")
    pub source: &'static str,
    /// Kind of completion for icon/styling
    pub kind: CompletionKind,
    /// Sort priority (lower = higher priority)
    pub sort_priority: u32,
    /// Custom filter text (defaults to label if None)
    pub filter_text: Option<String>,
    /// Score from fuzzy matching (set by completion engine)
    pub score: u32,
}

impl CompletionItem {
    /// Create a new completion item with minimal data
    #[must_use]
    pub fn new(insert_text: impl Into<String>, source: &'static str) -> Self {
        let text = insert_text.into();
        Self {
            label: text.clone(),
            insert_text: text,
            detail: None,
            documentation: None,
            source,
            kind: CompletionKind::Text,
            sort_priority: 100,
            filter_text: None,
            score: 0,
        }
    }

    /// Set custom label
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set detail text
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set documentation
    #[must_use]
    pub fn with_documentation(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }

    /// Set completion kind
    #[must_use]
    pub const fn with_kind(mut self, kind: CompletionKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set sort priority
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.sort_priority = priority;
        self
    }

    /// Set filter text
    #[must_use]
    pub fn with_filter_text(mut self, text: impl Into<String>) -> Self {
        self.filter_text = Some(text.into());
        self
    }

    /// Get the text to use for filtering
    #[must_use]
    pub fn filter_text(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }
}

/// Context for completion requests
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// Buffer ID being completed
    pub buffer_id: usize,
    /// File path (for language detection)
    pub file_path: Option<String>,
    /// Current cursor row (0-indexed)
    pub cursor_row: u32,
    /// Current cursor column (0-indexed)
    pub cursor_col: u32,
    /// Current line content
    pub line: String,
    /// Prefix text being completed
    pub prefix: String,
    /// Column where the word being completed starts
    pub word_start_col: u32,
    /// Character that triggered completion (if any)
    pub trigger_char: Option<char>,
}

impl CompletionContext {
    /// Create a new completion context
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String parameters aren't const constructible
    pub fn new(
        buffer_id: usize,
        cursor_row: u32,
        cursor_col: u32,
        line: String,
        prefix: String,
        word_start_col: u32,
    ) -> Self {
        Self {
            buffer_id,
            file_path: None,
            cursor_row,
            cursor_col,
            line,
            prefix,
            word_start_col,
            trigger_char: None,
        }
    }

    /// Set file path
    #[must_use]
    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Set trigger character
    #[must_use]
    pub const fn with_trigger_char(mut self, ch: char) -> Self {
        self.trigger_char = Some(ch);
        self
    }
}

/// Abstract completion provider
///
/// Plugins implement this trait to provide completion functionality.
/// The runtime uses this provider to fetch completions for buffers.
pub trait CompletionProvider: Send + Sync {
    /// Check if this provider is available for the given context
    fn is_available(&self, ctx: &CompletionContext) -> bool;

    /// Request completion to start
    ///
    /// This triggers an async completion request. Results are delivered
    /// via the completion cache and render signal.
    fn request_completion(&self, ctx: CompletionContext);

    /// Dismiss active completion
    fn dismiss(&self);

    /// Check if completion is currently active
    fn is_active(&self) -> bool;
}

/// Factory for creating completion providers
///
/// Plugins implement this trait to provide completion for files.
/// The runtime uses this factory to obtain the completion provider.
pub trait CompletionFactory: Send + Sync {
    /// Get the completion provider
    ///
    /// Returns the shared completion provider instance.
    /// Unlike syntax which is per-buffer, completion is typically global.
    fn get_provider(&self) -> Arc<dyn CompletionProvider>;
}

/// Shared reference to a completion factory
pub type SharedCompletionFactory = Arc<dyn CompletionFactory>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_kind_default() {
        let kind = CompletionKind::default();
        assert_eq!(kind, CompletionKind::Text);
    }

    #[test]
    fn test_completion_kind_variants() {
        // Just ensure all variants exist and are distinct
        let kinds = [
            CompletionKind::Text,
            CompletionKind::Method,
            CompletionKind::Function,
            CompletionKind::Constructor,
            CompletionKind::Field,
            CompletionKind::Variable,
            CompletionKind::Class,
            CompletionKind::Interface,
            CompletionKind::Module,
            CompletionKind::Property,
            CompletionKind::Unit,
            CompletionKind::Value,
            CompletionKind::Enum,
            CompletionKind::Keyword,
            CompletionKind::Snippet,
            CompletionKind::Color,
            CompletionKind::File,
            CompletionKind::Reference,
            CompletionKind::Folder,
            CompletionKind::EnumMember,
            CompletionKind::Constant,
            CompletionKind::Struct,
            CompletionKind::Event,
            CompletionKind::Operator,
            CompletionKind::TypeParameter,
        ];
        assert_eq!(kinds.len(), 25);
    }

    #[test]
    fn test_completion_item_new() {
        let item = CompletionItem::new("test_text", "test_source");

        assert_eq!(item.insert_text, "test_text");
        assert_eq!(item.label, "test_text");
        assert_eq!(item.source, "test_source");
        assert_eq!(item.kind, CompletionKind::Text);
        assert_eq!(item.sort_priority, 100);
        assert!(item.detail.is_none());
        assert!(item.documentation.is_none());
        assert!(item.filter_text.is_none());
        assert_eq!(item.score, 0);
    }

    #[test]
    fn test_completion_item_with_label() {
        let item = CompletionItem::new("insert", "source").with_label("Display Label");

        assert_eq!(item.insert_text, "insert");
        assert_eq!(item.label, "Display Label");
    }

    #[test]
    fn test_completion_item_with_detail() {
        let item = CompletionItem::new("func", "source").with_detail("fn() -> i32");

        assert_eq!(item.detail, Some("fn() -> i32".to_string()));
    }

    #[test]
    fn test_completion_item_with_documentation() {
        let item = CompletionItem::new("func", "source")
            .with_documentation("This function does something useful.");

        assert_eq!(item.documentation, Some("This function does something useful.".to_string()));
    }

    #[test]
    fn test_completion_item_with_kind() {
        let item = CompletionItem::new("func", "source").with_kind(CompletionKind::Function);

        assert_eq!(item.kind, CompletionKind::Function);
    }

    #[test]
    fn test_completion_item_with_priority() {
        let item = CompletionItem::new("item", "source").with_priority(50);

        assert_eq!(item.sort_priority, 50);
    }

    #[test]
    fn test_completion_item_with_filter_text() {
        let item = CompletionItem::new("item", "source").with_filter_text("custom_filter");

        assert_eq!(item.filter_text, Some("custom_filter".to_string()));
    }

    #[test]
    fn test_completion_item_filter_text_accessor() {
        // Without custom filter text, should return label
        let item1 = CompletionItem::new("label_text", "source");
        assert_eq!(item1.filter_text(), "label_text");

        // With custom filter text, should return it
        let item2 = CompletionItem::new("label", "source").with_filter_text("filter");
        assert_eq!(item2.filter_text(), "filter");
    }

    #[test]
    fn test_completion_item_builder_chain() {
        let item = CompletionItem::new("complete", "lsp")
            .with_label("complete()")
            .with_detail("fn complete() -> Result<()>")
            .with_documentation("Completes the operation")
            .with_kind(CompletionKind::Function)
            .with_priority(10)
            .with_filter_text("comp");

        assert_eq!(item.insert_text, "complete");
        assert_eq!(item.label, "complete()");
        assert_eq!(item.detail, Some("fn complete() -> Result<()>".to_string()));
        assert_eq!(item.documentation, Some("Completes the operation".to_string()));
        assert_eq!(item.kind, CompletionKind::Function);
        assert_eq!(item.sort_priority, 10);
        assert_eq!(item.filter_text, Some("comp".to_string()));
        assert_eq!(item.source, "lsp");
    }

    #[test]
    fn test_completion_context_new() {
        let ctx =
            CompletionContext::new(42, 10, 15, "let x = some".to_string(), "some".to_string(), 8);

        assert_eq!(ctx.buffer_id, 42);
        assert_eq!(ctx.cursor_row, 10);
        assert_eq!(ctx.cursor_col, 15);
        assert_eq!(ctx.line, "let x = some");
        assert_eq!(ctx.prefix, "some");
        assert_eq!(ctx.word_start_col, 8);
        assert!(ctx.file_path.is_none());
        assert!(ctx.trigger_char.is_none());
    }

    #[test]
    fn test_completion_context_with_file_path() {
        let ctx = CompletionContext::new(0, 0, 0, String::new(), String::new(), 0)
            .with_file_path("/path/to/file.rs");

        assert_eq!(ctx.file_path, Some("/path/to/file.rs".to_string()));
    }

    #[test]
    fn test_completion_context_with_trigger_char() {
        let ctx =
            CompletionContext::new(0, 0, 0, String::new(), String::new(), 0).with_trigger_char('.');

        assert_eq!(ctx.trigger_char, Some('.'));
    }

    #[test]
    fn test_completion_context_full_builder() {
        let ctx = CompletionContext::new(1, 5, 10, "text".to_string(), "pre".to_string(), 7)
            .with_file_path("main.rs")
            .with_trigger_char(':');

        assert_eq!(ctx.buffer_id, 1);
        assert_eq!(ctx.cursor_row, 5);
        assert_eq!(ctx.cursor_col, 10);
        assert_eq!(ctx.line, "text");
        assert_eq!(ctx.prefix, "pre");
        assert_eq!(ctx.word_start_col, 7);
        assert_eq!(ctx.file_path, Some("main.rs".to_string()));
        assert_eq!(ctx.trigger_char, Some(':'));
    }

    #[test]
    fn test_completion_item_clone() {
        let item = CompletionItem::new("test", "source")
            .with_label("label")
            .with_detail("detail");

        let cloned = item.clone();

        assert_eq!(cloned.insert_text, item.insert_text);
        assert_eq!(cloned.label, item.label);
        assert_eq!(cloned.detail, item.detail);
    }

    #[test]
    fn test_completion_context_clone() {
        let ctx = CompletionContext::new(1, 2, 3, "line".to_string(), "p".to_string(), 0)
            .with_file_path("/file");

        let cloned = ctx.clone();

        assert_eq!(cloned.buffer_id, ctx.buffer_id);
        assert_eq!(cloned.file_path, ctx.file_path);
    }

    #[test]
    fn test_completion_item_debug() {
        let item = CompletionItem::new("test", "source");
        let debug_str = format!("{item:?}");

        assert!(debug_str.contains("CompletionItem"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_completion_context_debug() {
        let ctx = CompletionContext::new(0, 0, 0, String::new(), String::new(), 0);
        let debug_str = format!("{ctx:?}");

        assert!(debug_str.contains("CompletionContext"));
    }

    #[test]
    fn test_completion_kind_debug() {
        let kind = CompletionKind::Function;
        let debug_str = format!("{kind:?}");

        assert!(debug_str.contains("Function"));
    }

    #[test]
    fn test_completion_kind_copy() {
        let kind1 = CompletionKind::Method;
        let kind2 = kind1; // Copy

        assert_eq!(kind1, kind2);
    }

    #[test]
    fn test_completion_kind_eq() {
        assert_eq!(CompletionKind::Text, CompletionKind::Text);
        assert_ne!(CompletionKind::Text, CompletionKind::Method);
    }
}
