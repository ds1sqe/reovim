//! Syntax highlight categories and spans.
//!
//! This module defines the highlight types used by syntax drivers.
//!
//! # Annotation Model (#540)
//!
//! The [`HighlightCategory`] type is an open string-based category.
//! Any provider can emit any category. Well-known categories exist
//! as constants for convenience, not constraint.
//!
//! [`Annotation`] extends byte-range spans with an [`AnnotationKind`] that
//! describes the visual effect (highlight, conceal, background, virtual text).

use std::{ops::Range, sync::Arc};

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
/// use reovim_driver_syntax::HighlightCategory;
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

/// What an annotation does visually.
///
/// Most syntax highlighting uses [`Highlight`](AnnotationKind::Highlight).
/// The other variants support decorations (conceal, background, virtual text)
/// that will be emitted by future providers (DAP, LSP, decoration queries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationKind {
    /// Style overlay — the common case for syntax highlighting.
    /// Client resolves category -> Style via `ThemeManager`.
    Highlight,

    /// Conceal the text range, optionally replacing with different text.
    /// `col_mapping` is computed client-side (rendering concern).
    Conceal {
        /// Replacement text, if any.
        replacement: Option<String>,
    },

    /// Background highlight (independent of text style).
    Background,

    /// Virtual text inserted at this position (not in buffer).
    VirtualText {
        /// The virtual text content.
        text: String,
    },
}

/// A single annotation on a buffer range.
///
/// Annotations are the unified representation for all visual markup:
/// syntax highlights, decorations, diagnostics, search matches, etc.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{Annotation, HighlightCategory};
///
/// let ann = Annotation::highlight(0, 5, HighlightCategory::new("keyword.function"));
/// assert_eq!(ann.category.as_str(), "keyword.function");
/// assert_eq!(ann.len(), 5);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// Start byte offset (inclusive).
    pub start_byte: usize,
    /// End byte offset (exclusive).
    pub end_byte: usize,
    /// Category string (e.g., "keyword.function", "dap.breakpoint").
    pub category: HighlightCategory,
    /// What this annotation does visually.
    pub kind: AnnotationKind,
}

impl Annotation {
    /// Create a highlight annotation (the common case).
    #[must_use]
    pub const fn highlight(
        start_byte: usize,
        end_byte: usize,
        category: HighlightCategory,
    ) -> Self {
        Self {
            start_byte,
            end_byte,
            category,
            kind: AnnotationKind::Highlight,
        }
    }

    /// Create an annotation with explicit kind.
    #[must_use]
    pub const fn new(
        start_byte: usize,
        end_byte: usize,
        category: HighlightCategory,
        kind: AnnotationKind,
    ) -> Self {
        Self {
            start_byte,
            end_byte,
            category,
            kind,
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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // HighlightCategory Tests
    // ========================================================================

    #[test]
    fn test_highlight_category_new_from_str() {
        let cat = HighlightCategory::new("keyword.function");
        assert_eq!(cat.as_str(), "keyword.function");
    }

    #[test]
    fn test_highlight_category_new_from_string() {
        let s = String::from("variable.builtin");
        let cat = HighlightCategory::new(s);
        assert_eq!(cat.as_str(), "variable.builtin");
    }

    #[test]
    fn test_highlight_category_new_from_arc_str() {
        let arc: Arc<str> = Arc::from("type.builtin");
        let cat = HighlightCategory::new(arc);
        assert_eq!(cat.as_str(), "type.builtin");
    }

    #[test]
    fn test_highlight_category_equality_same_arc() {
        let cat1 = HighlightCategory::new("keyword");
        let cat2 = cat1.clone();
        assert_eq!(cat1, cat2);
    }

    #[test]
    fn test_highlight_category_equality_different_arcs() {
        let cat1 = HighlightCategory::new(String::from("keyword"));
        let cat2 = HighlightCategory::new(String::from("keyword"));
        assert_eq!(cat1, cat2);
    }

    #[test]
    fn test_highlight_category_inequality() {
        let cat1 = HighlightCategory::new("keyword");
        let cat2 = HighlightCategory::new("function");
        assert_ne!(cat1, cat2);
    }

    #[test]
    fn test_highlight_category_hash_consistency() {
        use std::collections::HashSet;
        let cat1 = HighlightCategory::new(String::from("keyword"));
        let cat2 = HighlightCategory::new(String::from("keyword"));
        let mut set = HashSet::new();
        set.insert(cat1);
        assert!(set.contains(&cat2));
    }

    #[test]
    fn test_highlight_category_display() {
        let cat = HighlightCategory::new("keyword.function");
        assert_eq!(format!("{cat}"), "keyword.function");
    }

    #[test]
    fn test_highlight_category_debug() {
        let cat = HighlightCategory::new("keyword");
        let debug = format!("{cat:?}");
        assert!(debug.contains("keyword"));
    }

    // ========================================================================
    // AnnotationKind Tests
    // ========================================================================

    #[test]
    fn test_annotation_kind_highlight() {
        let kind = AnnotationKind::Highlight;
        assert_eq!(kind, AnnotationKind::Highlight);
    }

    #[test]
    fn test_annotation_kind_conceal_none() {
        let kind = AnnotationKind::Conceal { replacement: None };
        assert!(matches!(kind, AnnotationKind::Conceal { replacement: None }));
    }

    #[test]
    fn test_annotation_kind_conceal_with_replacement() {
        let kind = AnnotationKind::Conceal {
            replacement: Some("*".into()),
        };
        assert!(matches!(
            kind,
            AnnotationKind::Conceal { replacement: Some(ref r) } if r == "*"
        ));
    }

    #[test]
    fn test_annotation_kind_background() {
        let kind = AnnotationKind::Background;
        assert_eq!(kind, AnnotationKind::Background);
    }

    #[test]
    fn test_annotation_kind_virtual_text() {
        let kind = AnnotationKind::VirtualText {
            text: "ghost".into(),
        };
        assert!(matches!(
            kind,
            AnnotationKind::VirtualText { ref text } if text == "ghost"
        ));
    }

    #[test]
    fn test_annotation_kind_clone() {
        let kind = AnnotationKind::Conceal {
            replacement: Some("x".into()),
        };
        let cloned = kind.clone();
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_annotation_kind_debug() {
        let kind = AnnotationKind::Highlight;
        let debug = format!("{kind:?}");
        assert!(debug.contains("Highlight"));
    }

    // ========================================================================
    // Annotation Tests
    // ========================================================================

    #[test]
    fn test_annotation_highlight_constructor() {
        let ann = Annotation::highlight(10, 20, HighlightCategory::new("keyword"));
        assert_eq!(ann.start_byte, 10);
        assert_eq!(ann.end_byte, 20);
        assert_eq!(ann.category.as_str(), "keyword");
        assert_eq!(ann.kind, AnnotationKind::Highlight);
    }

    #[test]
    fn test_annotation_new_with_kind() {
        let ann = Annotation::new(
            0,
            5,
            HighlightCategory::new("decoration.heading"),
            AnnotationKind::Conceal {
                replacement: Some("*".into()),
            },
        );
        assert_eq!(ann.start_byte, 0);
        assert_eq!(ann.end_byte, 5);
        assert_eq!(ann.category.as_str(), "decoration.heading");
        assert!(matches!(ann.kind, AnnotationKind::Conceal { .. }));
    }

    #[test]
    fn test_annotation_len() {
        let ann = Annotation::highlight(10, 20, HighlightCategory::new("keyword"));
        assert_eq!(ann.len(), 10);
    }

    #[test]
    fn test_annotation_is_empty() {
        let ann = Annotation::highlight(5, 5, HighlightCategory::new("keyword"));
        assert!(ann.is_empty());

        let ann = Annotation::highlight(5, 10, HighlightCategory::new("keyword"));
        assert!(!ann.is_empty());
    }

    #[test]
    fn test_annotation_overlaps() {
        let ann = Annotation::highlight(10, 20, HighlightCategory::new("keyword"));
        assert!(ann.overlaps(&(5..15)));
        assert!(ann.overlaps(&(15..25)));
        assert!(ann.overlaps(&(12..18)));
        assert!(!ann.overlaps(&(0..10)));
        assert!(!ann.overlaps(&(20..30)));
    }

    #[test]
    fn test_annotation_contains() {
        let ann = Annotation::highlight(10, 20, HighlightCategory::new("keyword"));
        assert!(ann.contains(10));
        assert!(ann.contains(15));
        assert!(ann.contains(19));
        assert!(!ann.contains(9));
        assert!(!ann.contains(20));
    }

    #[test]
    fn test_annotation_byte_range() {
        let ann = Annotation::highlight(10, 20, HighlightCategory::new("keyword"));
        assert_eq!(ann.byte_range(), 10..20);
    }

    #[test]
    fn test_annotation_clone() {
        let ann = Annotation::highlight(0, 5, HighlightCategory::new("keyword"));
        let cloned = ann.clone();
        assert_eq!(ann, cloned);
    }

    #[test]
    fn test_annotation_debug() {
        let ann = Annotation::highlight(0, 5, HighlightCategory::new("keyword"));
        let debug = format!("{ann:?}");
        assert!(debug.contains("Annotation"));
        assert!(debug.contains("keyword"));
    }

    #[test]
    fn test_annotation_equality() {
        let a1 = Annotation::highlight(0, 5, HighlightCategory::new("keyword"));
        let a2 = Annotation::highlight(0, 5, HighlightCategory::new("keyword"));
        let a3 = Annotation::highlight(0, 5, HighlightCategory::new("function"));
        assert_eq!(a1, a2);
        assert_ne!(a1, a3);
    }
}
