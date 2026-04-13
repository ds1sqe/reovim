//! Decoration types for the syntax driver layer.
//!
//! This module provides types for declarative decoration rules that map
//! tree-sitter capture names to semantic categories. Language modules
//! provide these rules as data; the driver applies them mechanically.
//!
//! # Design
//!
//! ```text
//! Lang module (policy)  →  DecorationRule[]  →  TreeSitterDriver (mechanism)
//!                                                    ↓
//!                          DecorationCapture[]  ←  runs query
//!                                    ↓
//!                          apply_rules()  →  Vec<Annotation>
//! ```
//!
//! No tree-sitter types appear here — this crate is parser-agnostic.

use std::sync::Arc;

use crate::{Annotation, HighlightCategory};

/// A declarative rule mapping a query capture name to a semantic category.
///
/// Language modules provide these to describe how tree-sitter captures
/// should be annotated. The driver applies them — no callbacks needed.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::{DecorationRule, HighlightCategory};
///
/// let rule = DecorationRule {
///     capture_name: "heading.1.marker".into(),
///     category: HighlightCategory::new("markup.heading.1"),
/// };
/// assert_eq!(rule.capture_name.as_ref(), "heading.1.marker");
/// ```
#[derive(Debug, Clone)]
pub struct DecorationRule {
    /// Capture name to match (e.g., `"heading.1.marker"`, `"code_block"`).
    pub capture_name: Arc<str>,
    /// Semantic category (e.g., `"markup.heading.1"`, `"markup.raw.block"`).
    pub category: HighlightCategory,
}

/// A raw capture from a decoration query (before rule application).
///
/// This is the intermediate representation between the query engine and
/// the final [`Annotation`]. Generic — no tree-sitter types.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::DecorationCapture;
///
/// let cap = DecorationCapture {
///     name: "heading.1.marker".into(),
///     start_byte: 0,
///     end_byte: 2,
/// };
/// assert_eq!(cap.name.as_ref(), "heading.1.marker");
/// assert_eq!(cap.byte_range(), 0..2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationCapture {
    /// Capture name from the query.
    pub name: Arc<str>,
    /// Start byte offset (inclusive).
    pub start_byte: usize,
    /// End byte offset (exclusive).
    pub end_byte: usize,
}

impl DecorationCapture {
    /// Get this capture as a byte range.
    #[must_use]
    pub const fn byte_range(&self) -> std::ops::Range<usize> {
        self.start_byte..self.end_byte
    }

    /// Get the length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end_byte - self.start_byte
    }

    /// Check if the capture is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }
}

/// Apply decoration rules to raw captures, producing annotations.
///
/// For each capture, finds the first rule whose `capture_name` matches
/// the capture's `name`, and produces an [`Annotation`] with the rule's
/// category. Captures with no matching rule are skipped.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::{
///     DecorationCapture, DecorationRule, HighlightCategory,
///     decoration::apply_rules,
/// };
///
/// let rules = vec![
///     DecorationRule {
///         capture_name: "code_block".into(),
///         category: HighlightCategory::new("markup.raw.block"),
///     },
/// ];
///
/// let captures = vec![
///     DecorationCapture { name: "code_block".into(), start_byte: 10, end_byte: 50 },
///     DecorationCapture { name: "unknown".into(), start_byte: 60, end_byte: 70 },
/// ];
///
/// let annotations = apply_rules(&captures, &rules);
/// assert_eq!(annotations.len(), 1);
/// assert_eq!(annotations[0].start_byte, 10);
/// ```
#[must_use]
pub fn apply_rules(captures: &[DecorationCapture], rules: &[DecorationRule]) -> Vec<Annotation> {
    captures
        .iter()
        .filter_map(|cap| {
            rules
                .iter()
                .find(|r| r.capture_name.as_ref() == cap.name.as_ref())
                .map(|rule| Annotation::new(cap.start_byte, cap.end_byte, rule.category.clone()))
        })
        .collect()
}

#[cfg(test)]
#[path = "decoration_tests.rs"]
mod tests;
