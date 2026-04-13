//! Syntax highlight categories and spans.
//!
//! This module defines the highlight types used by syntax drivers.
//!
//! # Annotation Model (#540, #551)
//!
//! The [`HighlightCategory`] type is an open string-based category.
//! Any provider can emit any category. Well-known categories exist
//! as constants for convenience, not constraint.
//!
//! [`Annotation`] is a pure semantic marker: byte range + category.
//! The server emits only WHAT to annotate; clients decide HOW to render.

use std::{fmt, ops::Range, sync::Arc};

// ============================================================================
// Annotation Model (#540)
// ============================================================================

/// Interned string-based highlight category.
///
/// Any provider can emit any category. Well-known categories exist
/// as constants for convenience, not constraint.
///
/// Uses `Arc<str>` for cheap cloning (O(1) vs `String`'s O(n)).
/// Different `Arc<str>` instances with the same content compare
/// equal via the [`PartialEq`] impl on the underlying `str`.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::HighlightCategory;
///
/// let cat = HighlightCategory::new("keyword.function");
/// assert_eq!(cat.as_str(), "keyword.function");
///
/// // Well-known constants
/// let kw = HighlightCategory::new(HighlightCategory::KEYWORD);
/// assert_eq!(kw.as_str(), "keyword");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HighlightCategory(Arc<str>);

impl HighlightCategory {
    /// Create a new highlight category from a string.
    #[must_use]
    pub fn new(s: impl Into<Arc<str>>) -> Self {
        Self(s.into())
    }

    /// Get the category string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HighlightCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Well-known category constants.
///
/// Use these for interop and readability — they are not a closed set.
impl HighlightCategory {
    // Keywords
    pub const KEYWORD: &str = "keyword";
    pub const KEYWORD_CONTROL: &str = "keyword.control";
    pub const KEYWORD_OPERATOR: &str = "keyword.operator";
    pub const KEYWORD_FUNCTION: &str = "keyword.function";
    pub const KEYWORD_TYPE: &str = "keyword.type";

    // Types
    pub const TYPE: &str = "type";
    pub const TYPE_BUILTIN: &str = "type.builtin";

    // Functions
    pub const FUNCTION: &str = "function";
    pub const FUNCTION_BUILTIN: &str = "function.builtin";
    pub const FUNCTION_MACRO: &str = "function.macro";
    pub const FUNCTION_METHOD: &str = "function.method";

    // Variables
    pub const VARIABLE: &str = "variable";
    pub const VARIABLE_BUILTIN: &str = "variable.builtin";
    pub const VARIABLE_PARAMETER: &str = "variable.parameter";
    pub const VARIABLE_FIELD: &str = "variable.field";
    pub const CONSTANT: &str = "constant";

    // Literals
    pub const STRING: &str = "string";
    pub const STRING_ESCAPE: &str = "string.escape";
    pub const CHARACTER: &str = "character";
    pub const NUMBER: &str = "number";
    pub const BOOLEAN: &str = "boolean";

    // Comments
    pub const COMMENT: &str = "comment";
    pub const COMMENT_DOC: &str = "comment.doc";

    // Punctuation
    pub const PUNCTUATION: &str = "punctuation";
    pub const PUNCTUATION_BRACKET: &str = "punctuation.bracket";
    pub const PUNCTUATION_DELIMITER: &str = "punctuation.delimiter";

    // Operators
    pub const OPERATOR: &str = "operator";

    // Diagnostics
    pub const DIAGNOSTIC_ERROR: &str = "diagnostic.error";
    pub const DIAGNOSTIC_WARNING: &str = "diagnostic.warning";
    pub const DIAGNOSTIC_INFO: &str = "diagnostic.info";
    pub const DIAGNOSTIC_HINT: &str = "diagnostic.hint";

    // Namespace / Constructor / Label / Attribute / Tag
    pub const NAMESPACE: &str = "namespace";
    pub const CONSTRUCTOR: &str = "constructor";
    pub const LABEL: &str = "label";
    pub const ATTRIBUTE: &str = "attribute";
    pub const TAG: &str = "tag";

    // Markup
    pub const MARKUP_HEADING: &str = "markup.heading";
    pub const MARKUP_BOLD: &str = "markup.bold";
    pub const MARKUP_ITALIC: &str = "markup.italic";
    pub const MARKUP_STRIKETHROUGH: &str = "markup.strikethrough";
    pub const MARKUP_LINK: &str = "markup.link";
    pub const MARKUP_LINK_URL: &str = "markup.link.url";
    pub const MARKUP_LIST: &str = "markup.list";
    pub const MARKUP_RAW: &str = "markup.raw";
    pub const MARKUP_RAW_INLINE: &str = "markup.raw.inline";

    // Embedded / Special
    pub const EMBEDDED: &str = "embedded";
    pub const SPECIAL: &str = "special";
}

/// A single annotation on a buffer range.
///
/// Annotations are the unified representation for all semantic markup:
/// syntax highlights, decorations, diagnostics, search matches, etc.
///
/// The server emits only byte ranges and categories (mechanism).
/// Clients decide how to render each category (policy).
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::{Annotation, HighlightCategory};
///
/// let ann = Annotation::new(0, 5, HighlightCategory::new("keyword.function"));
/// assert_eq!(ann.category.as_str(), "keyword.function");
/// assert_eq!(ann.len(), 5);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// Start byte offset (inclusive).
    pub start_byte: usize,
    /// End byte offset (exclusive).
    pub end_byte: usize,
    /// Category string (e.g., "keyword.function", "markup.heading.1").
    pub category: HighlightCategory,
}

impl Annotation {
    /// Create an annotation.
    #[must_use]
    pub const fn new(start_byte: usize, end_byte: usize, category: HighlightCategory) -> Self {
        Self {
            start_byte,
            end_byte,
            category,
        }
    }

    /// Get the length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end_byte - self.start_byte
    }

    /// Check if the annotation is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }

    /// Check if this annotation overlaps with a byte range.
    #[must_use]
    pub const fn overlaps(&self, range: &Range<usize>) -> bool {
        self.start_byte < range.end && self.end_byte > range.start
    }

    /// Check if this annotation contains a byte offset.
    #[must_use]
    pub const fn contains(&self, byte: usize) -> bool {
        self.start_byte <= byte && byte < self.end_byte
    }

    /// Get this annotation as a byte range.
    #[must_use]
    pub const fn byte_range(&self) -> Range<usize> {
        self.start_byte..self.end_byte
    }
}

// ============================================================================
// Syntax Context
// ============================================================================

/// Syntax context at a byte position.
///
/// Used by consumers (e.g., auto-pair) to determine if a position
/// is inside a string literal, comment, or normal code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxContext {
    /// Normal code (default for unparsed content).
    Code,
    /// Inside a string literal.
    String,
    /// Inside a comment.
    Comment,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl fmt::Display for SyntaxContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code => f.write_str("code"),
            Self::String => f.write_str("string"),
            Self::Comment => f.write_str("comment"),
        }
    }
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
