//! Language metadata and registry types.
//!
//! This module defines types for language metadata (file extensions, MIME types,
//! comment syntax) and the [`LanguageRegistry`] trait for language detection.

/// Information about a supported language.
///
/// Contains metadata-only information about a language, not parsing capabilities.
/// Used for language detection and editor configuration.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{LanguageInfo, CommentTokens};
///
/// let rust = LanguageInfo::new("rust", "Rust")
///     .with_extensions(["rs"])
///     .with_mime_types(["text/x-rust"])
///     .with_comments(CommentTokens::with_block("//", "/*", "*/"));
///
/// assert!(rust.matches_extension("rs"));
/// assert!(rust.matches_extension("RS")); // Case insensitive
/// ```
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    /// Language identifier (e.g., "rust", "python").
    pub id: String,
    /// Human-readable name (e.g., "Rust", "Python").
    pub name: String,
    /// File extensions (without dot, e.g., `["rs"]`, `["py", "pyw"]`).
    pub extensions: Vec<String>,
    /// MIME types (e.g., `["text/x-rust"]`).
    pub mime_types: Vec<String>,
    /// Comment tokens for this language.
    pub comment_tokens: CommentTokens,
}

impl LanguageInfo {
    /// Create new language info with minimal fields.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            extensions: Vec::new(),
            mime_types: Vec::new(),
            comment_tokens: CommentTokens::default(),
        }
    }

    /// Add file extensions.
    #[must_use]
    pub fn with_extensions<I, S>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.extensions = extensions.into_iter().map(Into::into).collect();
        self
    }

    /// Add MIME types.
    #[must_use]
    pub fn with_mime_types<I, S>(mut self, mime_types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.mime_types = mime_types.into_iter().map(Into::into).collect();
        self
    }

    /// Set comment tokens.
    #[must_use]
    pub fn with_comments(mut self, tokens: CommentTokens) -> Self {
        self.comment_tokens = tokens;
        self
    }

    /// Check if this language matches a file extension.
    ///
    /// Comparison is case-insensitive.
    #[must_use]
    pub fn matches_extension(&self, ext: &str) -> bool {
        self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
    }

    /// Check if this language matches a MIME type.
    ///
    /// Comparison is case-insensitive.
    #[must_use]
    pub fn matches_mime(&self, mime: &str) -> bool {
        self.mime_types.iter().any(|m| m.eq_ignore_ascii_case(mime))
    }

    /// Check if this language has any registered extensions.
    #[must_use]
    pub const fn has_extensions(&self) -> bool {
        !self.extensions.is_empty()
    }

    /// Check if this language has comment syntax defined.
    #[must_use]
    pub const fn has_comments(&self) -> bool {
        self.comment_tokens.has_any()
    }
}

/// Comment syntax for a language.
///
/// Describes how to create line and block comments.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::CommentTokens;
///
/// // C-style comments
/// let c_style = CommentTokens::with_block("//", "/*", "*/");
/// assert!(c_style.has_line_comment());
/// assert!(c_style.has_block_comment());
///
/// // Python-style (line only)
/// let python = CommentTokens::line_only("#");
/// assert!(python.has_line_comment());
/// assert!(!python.has_block_comment());
///
/// // HTML-style (block only)
/// let html = CommentTokens::block_only("<!--", "-->");
/// assert!(!html.has_line_comment());
/// assert!(html.has_block_comment());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentTokens {
    /// Line comment prefix (e.g., "//", "#", "--").
    pub line: Option<String>,
    /// Block comment start (e.g., "/*", "<!--").
    pub block_start: Option<String>,
    /// Block comment end (e.g., "*/", "-->").
    pub block_end: Option<String>,
}

impl CommentTokens {
    /// Create comment tokens with only a line comment.
    #[must_use]
    pub fn line_only(prefix: impl Into<String>) -> Self {
        Self {
            line: Some(prefix.into()),
            block_start: None,
            block_end: None,
        }
    }

    /// Create comment tokens with both line and block comments.
    #[must_use]
    pub fn with_block(
        line: impl Into<String>,
        block_start: impl Into<String>,
        block_end: impl Into<String>,
    ) -> Self {
        Self {
            line: Some(line.into()),
            block_start: Some(block_start.into()),
            block_end: Some(block_end.into()),
        }
    }

    /// Create comment tokens with only block comments (no line comments).
    #[must_use]
    pub fn block_only(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            line: None,
            block_start: Some(start.into()),
            block_end: Some(end.into()),
        }
    }

    /// Check if line comments are supported.
    #[must_use]
    pub const fn has_line_comment(&self) -> bool {
        self.line.is_some()
    }

    /// Check if block comments are supported.
    #[must_use]
    pub const fn has_block_comment(&self) -> bool {
        self.block_start.is_some() && self.block_end.is_some()
    }

    /// Check if any comment syntax is defined.
    #[must_use]
    pub const fn has_any(&self) -> bool {
        self.line.is_some() || (self.block_start.is_some() && self.block_end.is_some())
    }
}

/// Registry for language metadata and detection.
///
/// Manages the mapping between file extensions, MIME types, and language IDs.
/// Does not create syntax drivers - that's the factory's job.
///
/// Implementations must be thread-safe (`Send + Sync`).
pub trait LanguageRegistry: Send + Sync {
    /// Detect language from a file path.
    ///
    /// Uses file extension to determine the language.
    /// Returns the language ID if detected.
    fn detect_from_path(&self, path: &str) -> Option<String>;

    /// Detect language from MIME type.
    ///
    /// Returns the language ID if a matching language is found.
    fn detect_from_mime(&self, mime: &str) -> Option<String>;

    /// Get language info by ID.
    fn get_info(&self, language_id: &str) -> Option<&LanguageInfo>;

    /// Check if a language is registered.
    fn is_registered(&self, language_id: &str) -> bool;

    /// List all registered language IDs.
    fn language_ids(&self) -> Vec<String>;

    /// Get the total number of registered languages.
    fn len(&self) -> usize {
        self.language_ids().len()
    }

    /// Check if the registry is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Concrete implementation of [`LanguageRegistry`] built from [`LanguageInfo`] entries.
///
/// Created at bootstrap from `LanguageInfoStore` entries registered by modules.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{DefaultLanguageRegistry, LanguageInfo, LanguageRegistry};
///
/// let registry = DefaultLanguageRegistry::new(vec![
///     LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
///     LanguageInfo::new("markdown", "Markdown").with_extensions(["md"]),
/// ]);
///
/// assert_eq!(registry.detect_from_path("main.rs"), Some("rust".to_string()));
/// assert_eq!(registry.detect_from_path("README.md"), Some("markdown".to_string()));
/// assert_eq!(registry.detect_from_path("file.txt"), None);
/// ```
pub struct DefaultLanguageRegistry {
    languages: Vec<LanguageInfo>,
}

impl DefaultLanguageRegistry {
    /// Create a registry from a list of language info entries.
    #[must_use]
    pub const fn new(languages: Vec<LanguageInfo>) -> Self {
        Self { languages }
    }
}

impl LanguageRegistry for DefaultLanguageRegistry {
    fn detect_from_path(&self, path: &str) -> Option<String> {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())?;
        self.languages
            .iter()
            .find(|info| info.matches_extension(ext))
            .map(|info| info.id.clone())
    }

    fn detect_from_mime(&self, mime: &str) -> Option<String> {
        self.languages
            .iter()
            .find(|info| info.matches_mime(mime))
            .map(|info| info.id.clone())
    }

    fn get_info(&self, language_id: &str) -> Option<&LanguageInfo> {
        self.languages.iter().find(|info| info.id == language_id)
    }

    fn is_registered(&self, language_id: &str) -> bool {
        self.languages.iter().any(|info| info.id == language_id)
    }

    fn language_ids(&self) -> Vec<String> {
        self.languages.iter().map(|info| info.id.clone()).collect()
    }
}

impl std::fmt::Debug for DefaultLanguageRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultLanguageRegistry")
            .field("languages", &self.language_ids())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_info_new() {
        let info = LanguageInfo::new("rust", "Rust");

        assert_eq!(info.id, "rust");
        assert_eq!(info.name, "Rust");
        assert!(info.extensions.is_empty());
        assert!(info.mime_types.is_empty());
        assert!(!info.has_comments());
    }

    #[test]
    fn test_language_info_with_extensions() {
        let info = LanguageInfo::new("rust", "Rust").with_extensions(["rs"]);

        assert_eq!(info.extensions, vec!["rs"]);
        assert!(info.has_extensions());
    }

    #[test]
    fn test_language_info_with_mime_types() {
        let info = LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"]);

        assert_eq!(info.mime_types, vec!["text/x-rust"]);
    }

    #[test]
    fn test_language_info_with_comments() {
        let info = LanguageInfo::new("rust", "Rust")
            .with_comments(CommentTokens::with_block("//", "/*", "*/"));

        assert!(info.has_comments());
        assert!(info.comment_tokens.has_line_comment());
        assert!(info.comment_tokens.has_block_comment());
    }

    #[test]
    fn test_language_info_matches_extension() {
        let info = LanguageInfo::new("rust", "Rust").with_extensions(["rs"]);

        assert!(info.matches_extension("rs"));
        assert!(info.matches_extension("RS")); // Case insensitive
        assert!(info.matches_extension("Rs"));
        assert!(!info.matches_extension("py"));
    }

    #[test]
    fn test_language_info_matches_mime() {
        let info = LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"]);

        assert!(info.matches_mime("text/x-rust"));
        assert!(info.matches_mime("TEXT/X-RUST")); // Case insensitive
        assert!(!info.matches_mime("text/plain"));
    }

    #[test]
    fn test_comment_tokens_line_only() {
        let tokens = CommentTokens::line_only("#");

        assert_eq!(tokens.line, Some("#".to_string()));
        assert_eq!(tokens.block_start, None);
        assert_eq!(tokens.block_end, None);
        assert!(tokens.has_line_comment());
        assert!(!tokens.has_block_comment());
        assert!(tokens.has_any());
    }

    #[test]
    fn test_comment_tokens_with_block() {
        let tokens = CommentTokens::with_block("//", "/*", "*/");

        assert_eq!(tokens.line, Some("//".to_string()));
        assert_eq!(tokens.block_start, Some("/*".to_string()));
        assert_eq!(tokens.block_end, Some("*/".to_string()));
        assert!(tokens.has_line_comment());
        assert!(tokens.has_block_comment());
        assert!(tokens.has_any());
    }

    #[test]
    fn test_comment_tokens_block_only() {
        let tokens = CommentTokens::block_only("<!--", "-->");

        assert_eq!(tokens.line, None);
        assert_eq!(tokens.block_start, Some("<!--".to_string()));
        assert_eq!(tokens.block_end, Some("-->".to_string()));
        assert!(!tokens.has_line_comment());
        assert!(tokens.has_block_comment());
        assert!(tokens.has_any());
    }

    #[test]
    fn test_comment_tokens_default() {
        let tokens = CommentTokens::default();

        assert_eq!(tokens.line, None);
        assert_eq!(tokens.block_start, None);
        assert_eq!(tokens.block_end, None);
        assert!(!tokens.has_line_comment());
        assert!(!tokens.has_block_comment());
        assert!(!tokens.has_any());
    }

    #[test]
    fn test_language_info_no_extensions() {
        let info = LanguageInfo::new("test", "Test");
        assert!(!info.has_extensions());
        assert!(!info.matches_extension("rs"));
    }

    #[test]
    fn test_language_info_no_mime_types() {
        let info = LanguageInfo::new("test", "Test");
        assert!(!info.matches_mime("text/plain"));
    }

    #[test]
    fn test_language_info_multiple_extensions() {
        let info = LanguageInfo::new("python", "Python").with_extensions(["py", "pyw", "pyi"]);

        assert!(info.matches_extension("py"));
        assert!(info.matches_extension("pyw"));
        assert!(info.matches_extension("pyi"));
        assert!(!info.matches_extension("rs"));
    }

    #[test]
    fn test_language_info_chaining() {
        let info = LanguageInfo::new("rust", "Rust")
            .with_extensions(["rs"])
            .with_mime_types(["text/x-rust"])
            .with_comments(CommentTokens::with_block("//", "/*", "*/"));

        assert_eq!(info.id, "rust");
        assert_eq!(info.name, "Rust");
        assert!(info.has_extensions());
        assert!(info.has_comments());
        assert!(info.matches_extension("rs"));
        assert!(info.matches_mime("text/x-rust"));
    }

    #[test]
    fn test_comment_tokens_equality() {
        let a = CommentTokens::line_only("//");
        let b = CommentTokens::line_only("//");
        let c = CommentTokens::line_only("#");

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_comment_tokens_partial_block() {
        // Only block_start without block_end should not count as block comment
        let tokens = CommentTokens {
            line: None,
            block_start: Some("/*".to_string()),
            block_end: None,
        };
        assert!(!tokens.has_block_comment());
        assert!(!tokens.has_any());
    }

    /// Simple registry for testing the trait.
    struct TestRegistry {
        languages: Vec<LanguageInfo>,
    }

    impl TestRegistry {
        fn new(languages: Vec<LanguageInfo>) -> Self {
            Self { languages }
        }
    }

    impl LanguageRegistry for TestRegistry {
        fn detect_from_path(&self, path: &str) -> Option<String> {
            let ext = path.rsplit('.').next()?;
            self.languages
                .iter()
                .find(|info| info.matches_extension(ext))
                .map(|info| info.id.clone())
        }

        fn detect_from_mime(&self, mime: &str) -> Option<String> {
            self.languages
                .iter()
                .find(|info| info.matches_mime(mime))
                .map(|info| info.id.clone())
        }

        fn get_info(&self, language_id: &str) -> Option<&LanguageInfo> {
            self.languages.iter().find(|info| info.id == language_id)
        }

        fn is_registered(&self, language_id: &str) -> bool {
            self.languages.iter().any(|info| info.id == language_id)
        }

        fn language_ids(&self) -> Vec<String> {
            self.languages.iter().map(|info| info.id.clone()).collect()
        }
    }

    #[test]
    fn test_language_registry_detect_from_path() {
        let registry = TestRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
            LanguageInfo::new("python", "Python").with_extensions(["py"]),
        ]);

        assert_eq!(registry.detect_from_path("main.rs"), Some("rust".to_string()));
        assert_eq!(registry.detect_from_path("script.py"), Some("python".to_string()));
        assert_eq!(registry.detect_from_path("file.txt"), None);
    }

    #[test]
    fn test_language_registry_detect_from_mime() {
        let registry = TestRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"]),
        ]);

        assert_eq!(registry.detect_from_mime("text/x-rust"), Some("rust".to_string()));
        assert_eq!(registry.detect_from_mime("text/plain"), None);
    }

    #[test]
    fn test_language_registry_get_info() {
        let registry = TestRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);

        let info = registry.get_info("rust");
        assert!(info.is_some());
        assert_eq!(info.unwrap().name, "Rust");

        assert!(registry.get_info("unknown").is_none());
    }

    #[test]
    fn test_language_registry_is_registered() {
        let registry = TestRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);

        assert!(registry.is_registered("rust"));
        assert!(!registry.is_registered("python"));
    }

    #[test]
    fn test_language_registry_len_and_empty() {
        let empty = TestRegistry::new(vec![]);
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let non_empty = TestRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);
        assert!(!non_empty.is_empty());
        assert_eq!(non_empty.len(), 1);
    }

    // ========================================================================
    // DefaultLanguageRegistry Tests
    // ========================================================================

    #[test]
    fn test_default_registry_detect_from_path() {
        let registry = DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
            LanguageInfo::new("markdown", "Markdown").with_extensions(["md", "markdown"]),
        ]);

        assert_eq!(registry.detect_from_path("main.rs"), Some("rust".to_string()));
        assert_eq!(registry.detect_from_path("README.md"), Some("markdown".to_string()));
        assert_eq!(registry.detect_from_path("notes.markdown"), Some("markdown".to_string()));
        assert_eq!(registry.detect_from_path("file.txt"), None);
    }

    #[test]
    fn test_default_registry_case_insensitive_extension() {
        let registry = DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        ]);

        assert_eq!(registry.detect_from_path("MAIN.RS"), Some("rust".to_string()));
        assert_eq!(registry.detect_from_path("lib.Rs"), Some("rust".to_string()));
    }

    #[test]
    fn test_default_registry_detect_from_path_no_extension() {
        let registry = DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        ]);

        assert_eq!(registry.detect_from_path("Makefile"), None);
        assert_eq!(registry.detect_from_path(""), None);
    }

    #[test]
    fn test_default_registry_detect_from_path_with_directories() {
        let registry = DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        ]);

        assert_eq!(
            registry.detect_from_path("/home/user/project/src/main.rs"),
            Some("rust".to_string())
        );
        assert_eq!(registry.detect_from_path("src/lib.rs"), Some("rust".to_string()));
    }

    #[test]
    fn test_default_registry_detect_from_mime() {
        let registry = DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"]),
        ]);

        assert_eq!(registry.detect_from_mime("text/x-rust"), Some("rust".to_string()));
        assert_eq!(registry.detect_from_mime("TEXT/X-RUST"), Some("rust".to_string()));
        assert_eq!(registry.detect_from_mime("text/plain"), None);
    }

    #[test]
    fn test_default_registry_get_info() {
        let registry = DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust")
                .with_extensions(["rs"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        ]);

        let info = registry.get_info("rust").unwrap();
        assert_eq!(info.id, "rust");
        assert_eq!(info.name, "Rust");
        assert!(info.has_comments());

        assert!(registry.get_info("python").is_none());
    }

    #[test]
    fn test_default_registry_is_registered() {
        let registry = DefaultLanguageRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);

        assert!(registry.is_registered("rust"));
        assert!(!registry.is_registered("python"));
    }

    #[test]
    fn test_default_registry_language_ids() {
        let registry = DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust"),
            LanguageInfo::new("markdown", "Markdown"),
        ]);

        let ids = registry.language_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"rust".to_string()));
        assert!(ids.contains(&"markdown".to_string()));
    }

    #[test]
    fn test_default_registry_len_and_empty() {
        let empty = DefaultLanguageRegistry::new(vec![]);
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let non_empty = DefaultLanguageRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);
        assert!(!non_empty.is_empty());
        assert_eq!(non_empty.len(), 1);
    }

    #[test]
    fn test_default_registry_debug() {
        let registry = DefaultLanguageRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);
        let debug = format!("{registry:?}");
        assert!(debug.contains("DefaultLanguageRegistry"));
        assert!(debug.contains("rust"));
    }
}
