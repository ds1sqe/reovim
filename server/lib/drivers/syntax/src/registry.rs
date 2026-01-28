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
}
