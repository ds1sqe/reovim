//! Syntax highlighting mechanism trait.
//!
//! Linux equivalent: Generic highlight categories (mechanism).
//! The actual highlight groups (policy) are defined in `lib/drivers/syntax/`.
//!
//! # Design Principle
//!
//! The kernel provides the *mechanism* (how highlights are categorized),
//! while drivers/modules provide the *policy* (what the categories are).

use std::{fmt::Debug, hash::Hash};

/// Trait for syntax highlight categories.
///
/// This is a mechanism trait that defines the interface for highlight types.
/// Implementations (policies) are provided by the syntax driver layer.
///
/// # Example
///
/// ```ignore
/// // In lib/drivers/syntax/
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// pub enum HighlightGroup {
///     Keyword,
///     Function,
///     String,
///     // ...
/// }
///
/// impl SyntaxHighlight for HighlightGroup {
///     fn category(&self) -> &'static str {
///         match self {
///             Self::Keyword => "keyword",
///             Self::Function => "function",
///             Self::String => "string",
///         }
///     }
/// }
/// ```
pub trait SyntaxHighlight: Debug + Copy + Eq + Hash + Send + Sync + 'static {
    /// Returns the category name for this highlight.
    ///
    /// Category names should be lowercase, hyphenated identifiers
    /// (e.g., "keyword", "function-builtin", "string-escape").
    fn category(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum TestHighlight {
        Keyword,
        Comment,
    }

    impl SyntaxHighlight for TestHighlight {
        fn category(&self) -> &'static str {
            match self {
                Self::Keyword => "keyword",
                Self::Comment => "comment",
            }
        }
    }

    #[test]
    fn test_syntax_highlight_trait() {
        let hl = TestHighlight::Keyword;
        assert_eq!(hl.category(), "keyword");

        let hl2 = TestHighlight::Comment;
        assert_eq!(hl2.category(), "comment");
    }

    #[test]
    fn test_syntax_highlight_equality() {
        let hl1 = TestHighlight::Keyword;
        let hl2 = TestHighlight::Keyword;
        let hl3 = TestHighlight::Comment;

        assert_eq!(hl1, hl2);
        assert_ne!(hl1, hl3);
    }

    #[test]
    fn test_syntax_highlight_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TestHighlight::Keyword);
        set.insert(TestHighlight::Comment);
        assert_eq!(set.len(), 2);
    }
}
