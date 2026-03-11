//! Decoration types for the syntax driver layer.
//!
//! This module provides types for declarative decoration rules that map
//! tree-sitter capture names to [`AnnotationKind`] variants. Language modules
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

use crate::{Annotation, AnnotationKind, HighlightCategory};

/// A declarative rule mapping a query capture name to an annotation.
///
/// Language modules provide these to describe how tree-sitter captures
/// should be rendered. The driver applies them — no callbacks needed.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{AnnotationKind, DecorationRule, HighlightCategory};
///
/// let rule = DecorationRule {
///     capture_name: "heading.1.marker".into(),
///     kind: AnnotationKind::Conceal { replacement: Some("# ".into()) },
///     category: HighlightCategory::new("markup.heading.1"),
/// };
/// assert_eq!(rule.capture_name.as_ref(), "heading.1.marker");
/// ```
#[derive(Debug, Clone)]
pub struct DecorationRule {
    /// Capture name to match (e.g., `"heading.1.marker"`, `"code_block"`).
    pub capture_name: Arc<str>,
    /// The visual effect to produce.
    pub kind: AnnotationKind,
    /// Theming category (e.g., `"markup.heading.1"`, `"markup.raw.block"`).
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
/// use reovim_driver_syntax::DecorationCapture;
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
/// kind and category. Captures with no matching rule are skipped.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{
///     AnnotationKind, DecorationCapture, DecorationRule, HighlightCategory,
///     decoration::apply_rules,
/// };
///
/// let rules = vec![
///     DecorationRule {
///         capture_name: "code_block".into(),
///         kind: AnnotationKind::Background,
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
/// assert_eq!(annotations[0].kind, AnnotationKind::Background);
/// ```
#[must_use]
pub fn apply_rules(captures: &[DecorationCapture], rules: &[DecorationRule]) -> Vec<Annotation> {
    captures
        .iter()
        .filter_map(|cap| {
            rules
                .iter()
                .find(|r| r.capture_name.as_ref() == cap.name.as_ref())
                .map(|rule| {
                    Annotation::new(
                        cap.start_byte,
                        cap.end_byte,
                        rule.category.clone(),
                        rule.kind.clone(),
                    )
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // DecorationRule Tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_decoration_rule_creation() {
        let rule = DecorationRule {
            capture_name: "heading.1.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f0965} ".into()),
            },
            category: HighlightCategory::new("markup.heading.1"),
        };

        assert_eq!(rule.capture_name.as_ref(), "heading.1.marker");
        assert_eq!(rule.category.as_str(), "markup.heading.1");
        assert!(matches!(
            rule.kind,
            AnnotationKind::Conceal {
                replacement: Some(ref r)
            } if r.contains('\u{f0965}')
        ));
    }

    #[test]
    fn test_decoration_rule_clone() {
        let rule = DecorationRule {
            capture_name: "code_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("markup.raw.block"),
        };
        #[allow(clippy::redundant_clone)]
        let cloned = rule.clone();
        assert_eq!(cloned.capture_name.as_ref(), "code_block");
        assert_eq!(cloned.kind, AnnotationKind::Background);
        assert_eq!(cloned.category.as_str(), "markup.raw.block");
    }

    #[test]
    fn test_decoration_rule_debug() {
        let rule = DecorationRule {
            capture_name: "test".into(),
            kind: AnnotationKind::Highlight,
            category: HighlightCategory::new("test"),
        };
        let debug = format!("{rule:?}");
        assert!(debug.contains("DecorationRule"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_decoration_rule_all_annotation_kinds() {
        // Highlight
        let rule = DecorationRule {
            capture_name: "hl".into(),
            kind: AnnotationKind::Highlight,
            category: HighlightCategory::new("test"),
        };
        assert_eq!(rule.kind, AnnotationKind::Highlight);

        // Conceal with replacement
        let rule = DecorationRule {
            capture_name: "con".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("x".into()),
            },
            category: HighlightCategory::new("test"),
        };
        assert!(matches!(rule.kind, AnnotationKind::Conceal { .. }));

        // Conceal without replacement
        let rule = DecorationRule {
            capture_name: "con2".into(),
            kind: AnnotationKind::Conceal { replacement: None },
            category: HighlightCategory::new("test"),
        };
        assert!(matches!(rule.kind, AnnotationKind::Conceal { replacement: None }));

        // Background
        let rule = DecorationRule {
            capture_name: "bg".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("test"),
        };
        assert_eq!(rule.kind, AnnotationKind::Background);

        // VirtualText
        let rule = DecorationRule {
            capture_name: "vt".into(),
            kind: AnnotationKind::VirtualText {
                text: "ghost".into(),
            },
            category: HighlightCategory::new("test"),
        };
        assert!(matches!(rule.kind, AnnotationKind::VirtualText { .. }));
    }

    // ========================================================================
    // DecorationCapture Tests
    // ========================================================================

    #[test]
    fn test_decoration_capture_creation() {
        let cap = DecorationCapture {
            name: "heading.1.marker".into(),
            start_byte: 0,
            end_byte: 2,
        };

        assert_eq!(cap.name.as_ref(), "heading.1.marker");
        assert_eq!(cap.start_byte, 0);
        assert_eq!(cap.end_byte, 2);
    }

    #[test]
    fn test_decoration_capture_byte_range() {
        let cap = DecorationCapture {
            name: "test".into(),
            start_byte: 10,
            end_byte: 20,
        };
        assert_eq!(cap.byte_range(), 10..20);
    }

    #[test]
    fn test_decoration_capture_len() {
        let cap = DecorationCapture {
            name: "test".into(),
            start_byte: 5,
            end_byte: 15,
        };
        assert_eq!(cap.len(), 10);
    }

    #[test]
    fn test_decoration_capture_is_empty() {
        let empty = DecorationCapture {
            name: "test".into(),
            start_byte: 5,
            end_byte: 5,
        };
        assert!(empty.is_empty());

        let non_empty = DecorationCapture {
            name: "test".into(),
            start_byte: 5,
            end_byte: 10,
        };
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_decoration_capture_clone() {
        let cap = DecorationCapture {
            name: "test".into(),
            start_byte: 0,
            end_byte: 5,
        };
        let cloned = cap.clone();
        assert_eq!(cap, cloned);
    }

    #[test]
    fn test_decoration_capture_equality() {
        let a = DecorationCapture {
            name: "test".into(),
            start_byte: 0,
            end_byte: 5,
        };
        let b = DecorationCapture {
            name: "test".into(),
            start_byte: 0,
            end_byte: 5,
        };
        let c = DecorationCapture {
            name: "other".into(),
            start_byte: 0,
            end_byte: 5,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_decoration_capture_debug() {
        let cap = DecorationCapture {
            name: "heading".into(),
            start_byte: 0,
            end_byte: 2,
        };
        let debug = format!("{cap:?}");
        assert!(debug.contains("DecorationCapture"));
        assert!(debug.contains("heading"));
    }

    // ========================================================================
    // apply_rules Tests
    // ========================================================================

    #[test]
    fn test_apply_rules_basic_match() {
        let rules = vec![DecorationRule {
            capture_name: "code_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("markup.raw.block"),
        }];

        let captures = vec![DecorationCapture {
            name: "code_block".into(),
            start_byte: 10,
            end_byte: 50,
        }];

        let annotations = apply_rules(&captures, &rules);
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].start_byte, 10);
        assert_eq!(annotations[0].end_byte, 50);
        assert_eq!(annotations[0].kind, AnnotationKind::Background);
        assert_eq!(annotations[0].category.as_str(), "markup.raw.block");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_apply_rules_conceal_with_replacement() {
        let rules = vec![DecorationRule {
            capture_name: "heading.1.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f0965} ".into()),
            },
            category: HighlightCategory::new("markup.heading.1"),
        }];

        let captures = vec![DecorationCapture {
            name: "heading.1.marker".into(),
            start_byte: 0,
            end_byte: 2,
        }];

        let annotations = apply_rules(&captures, &rules);
        assert_eq!(annotations.len(), 1);
        assert!(matches!(
            &annotations[0].kind,
            AnnotationKind::Conceal { replacement: Some(r) } if r.contains('\u{f0965}')
        ));
    }

    #[test]
    fn test_apply_rules_skips_unmatched_captures() {
        let rules = vec![DecorationRule {
            capture_name: "code_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("markup.raw.block"),
        }];

        let captures = vec![
            DecorationCapture {
                name: "code_block".into(),
                start_byte: 10,
                end_byte: 50,
            },
            DecorationCapture {
                name: "unknown_capture".into(),
                start_byte: 60,
                end_byte: 70,
            },
        ];

        let annotations = apply_rules(&captures, &rules);
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].start_byte, 10);
    }

    #[test]
    fn test_apply_rules_empty_captures() {
        let rules = vec![DecorationRule {
            capture_name: "code_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("markup.raw.block"),
        }];

        let annotations = apply_rules(&[], &rules);
        assert!(annotations.is_empty());
    }

    #[test]
    fn test_apply_rules_empty_rules() {
        let captures = vec![DecorationCapture {
            name: "code_block".into(),
            start_byte: 10,
            end_byte: 50,
        }];

        let annotations = apply_rules(&captures, &[]);
        assert!(annotations.is_empty());
    }

    #[test]
    fn test_apply_rules_both_empty() {
        let annotations = apply_rules(&[], &[]);
        assert!(annotations.is_empty());
    }

    #[test]
    fn test_apply_rules_multiple_rules_multiple_captures() {
        let rules = vec![
            DecorationRule {
                capture_name: "heading.1.marker".into(),
                kind: AnnotationKind::Conceal {
                    replacement: Some("H1".into()),
                },
                category: HighlightCategory::new("markup.heading.1"),
            },
            DecorationRule {
                capture_name: "code_block".into(),
                kind: AnnotationKind::Background,
                category: HighlightCategory::new("markup.raw.block"),
            },
            DecorationRule {
                capture_name: "list.bullet".into(),
                kind: AnnotationKind::Conceal {
                    replacement: Some("\u{2022}".into()),
                },
                category: HighlightCategory::new("markup.list"),
            },
        ];

        let captures = vec![
            DecorationCapture {
                name: "heading.1.marker".into(),
                start_byte: 0,
                end_byte: 2,
            },
            DecorationCapture {
                name: "code_block".into(),
                start_byte: 20,
                end_byte: 60,
            },
            DecorationCapture {
                name: "list.bullet".into(),
                start_byte: 70,
                end_byte: 72,
            },
        ];

        let annotations = apply_rules(&captures, &rules);
        assert_eq!(annotations.len(), 3);

        assert_eq!(annotations[0].category.as_str(), "markup.heading.1");
        assert!(matches!(annotations[0].kind, AnnotationKind::Conceal { .. }));

        assert_eq!(annotations[1].category.as_str(), "markup.raw.block");
        assert_eq!(annotations[1].kind, AnnotationKind::Background);

        assert_eq!(annotations[2].category.as_str(), "markup.list");
        assert!(matches!(annotations[2].kind, AnnotationKind::Conceal { .. }));
    }

    #[test]
    fn test_apply_rules_first_matching_rule_wins() {
        let rules = vec![
            DecorationRule {
                capture_name: "test".into(),
                kind: AnnotationKind::Background,
                category: HighlightCategory::new("first"),
            },
            DecorationRule {
                capture_name: "test".into(),
                kind: AnnotationKind::Highlight,
                category: HighlightCategory::new("second"),
            },
        ];

        let captures = vec![DecorationCapture {
            name: "test".into(),
            start_byte: 0,
            end_byte: 10,
        }];

        let annotations = apply_rules(&captures, &rules);
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].category.as_str(), "first");
        assert_eq!(annotations[0].kind, AnnotationKind::Background);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_apply_rules_virtual_text() {
        let rules = vec![DecorationRule {
            capture_name: "hint".into(),
            kind: AnnotationKind::VirtualText {
                text: " // inferred: i32".into(),
            },
            category: HighlightCategory::new("hint.type"),
        }];

        let captures = vec![DecorationCapture {
            name: "hint".into(),
            start_byte: 5,
            end_byte: 5,
        }];

        let annotations = apply_rules(&captures, &rules);
        assert_eq!(annotations.len(), 1);
        assert!(matches!(
            &annotations[0].kind,
            AnnotationKind::VirtualText { text } if text == " // inferred: i32"
        ));
    }

    #[test]
    fn test_apply_rules_preserves_capture_order() {
        let rules = vec![
            DecorationRule {
                capture_name: "a".into(),
                kind: AnnotationKind::Background,
                category: HighlightCategory::new("a"),
            },
            DecorationRule {
                capture_name: "b".into(),
                kind: AnnotationKind::Background,
                category: HighlightCategory::new("b"),
            },
        ];

        let captures = vec![
            DecorationCapture {
                name: "b".into(),
                start_byte: 0,
                end_byte: 5,
            },
            DecorationCapture {
                name: "a".into(),
                start_byte: 10,
                end_byte: 15,
            },
        ];

        let annotations = apply_rules(&captures, &rules);
        assert_eq!(annotations.len(), 2);
        // Order preserved from captures, not rules
        assert_eq!(annotations[0].category.as_str(), "b");
        assert_eq!(annotations[1].category.as_str(), "a");
    }
}
