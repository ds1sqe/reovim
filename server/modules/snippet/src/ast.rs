//! Snippet AST types covering the full VSCode/LSP EBNF grammar (#136).
//!
//! All element types are active: `Text`, `TabStop`, `Placeholder`,
//! `Variable`, `Choice`, and `Transform`.
//!
//! # EBNF Reference
//!
//! ```text
//! any         ::= tabstop | placeholder | choice | variable | text
//! tabstop     ::= '$' int | '${' int '}' | '${' int transform '}'
//! placeholder ::= '${' int ':' any '}'
//! choice      ::= '${' int '|' text (',' text)* '|}'
//! variable    ::= '$' var | '${' var '}' | '${' var ':' any '}' | '${' var transform '}'
//! ```

use std::fmt;

/// Tab stop identifier. 0 is the final cursor position, 1..N are numbered stops.
pub type TabStopId = u32;

/// A single element in a snippet body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetElement {
    /// Literal text.
    Text(String),

    /// A tab stop: `$1`, `${1}`, or `${1/regex/replace/flags}`.
    TabStop {
        /// Tab stop number (0 = final position).
        id: TabStopId,
        /// Regex transform applied to the tab stop value.
        transform: Option<Transform>,
    },

    /// A placeholder with default content: `${1:default}`.
    Placeholder {
        /// Tab stop number this placeholder belongs to.
        id: TabStopId,
        /// Nested snippet elements (supports recursive placeholders).
        body: Vec<Self>,
    },

    /// A variable reference: `$TM_FILENAME`, `${VAR:default}`.
    Variable {
        /// Variable name (e.g., `TM_FILENAME`, `CURRENT_YEAR`).
        name: String,
        /// Default value if variable is unset.
        default: Option<Vec<Self>>,
        /// Regex transform applied to the variable value.
        transform: Option<Transform>,
    },

    /// A choice: `${1|one,two,three|}`.
    Choice {
        /// Tab stop number.
        id: TabStopId,
        /// Available choices.
        choices: Vec<String>,
    },
}

/// Regex transform: `/${regex}/${replacement}/${options}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transform {
    /// Regex pattern string.
    pub regex: String,
    /// Replacement items (mix of literals and capture references).
    pub replacement: Vec<FormatItem>,
    /// Regex flags (e.g., "g", "i").
    pub options: String,
}

/// A single item in a transform replacement string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatItem {
    /// Literal text in replacement.
    Text(String),

    /// Capture group reference: `$1`, `${1}`.
    Capture(usize),

    /// Case-changing capture: `${1:/upcase}`.
    CaseChange(usize, CaseModifier),

    /// Conditional insertion: `${1:+if}`, `${1:?if:else}`, `${1:-else}`.
    Conditional {
        /// Capture group number.
        capture: usize,
        /// Text to insert if capture matched.
        if_text: String,
        /// Text to insert if capture did not match.
        else_text: String,
    },
}

/// Case modifier for transform format items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseModifier {
    /// `/upcase` - convert to uppercase.
    Upcase,
    /// `/downcase` - convert to lowercase.
    Downcase,
    /// `/capitalize` - capitalize first letter.
    Capitalize,
    /// `/camelcase` - convert to camelCase.
    CamelCase,
    /// `/pascalcase` - convert to `PascalCase`.
    PascalCase,
    /// `/snakecase` - convert to `snake_case`.
    SnakeCase,
    /// `/kebabcase` - convert to kebab-case.
    KebabCase,
}

/// A parsed snippet body: a sequence of snippet elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetBody(pub Vec<SnippetElement>);

impl SnippetBody {
    /// Create a new snippet body from elements.
    #[must_use]
    pub const fn new(elements: Vec<SnippetElement>) -> Self {
        Self(elements)
    }

    /// Get the elements as a slice.
    #[must_use]
    pub fn elements(&self) -> &[SnippetElement] {
        &self.0
    }

    /// Check if the body is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of top-level elements.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Display for SnippetElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => write!(f, "{text}"),
            Self::TabStop { id, transform: _ } => write!(f, "${{{id}}}"),
            Self::Placeholder { id, body } => {
                write!(f, "${{{id}:")?;
                for elem in body {
                    write!(f, "{elem}")?;
                }
                write!(f, "}}")
            }
            Self::Variable {
                name,
                default,
                transform: _,
            } => {
                if let Some(default) = default {
                    write!(f, "${{{name}:")?;
                    for elem in default {
                        write!(f, "{elem}")?;
                    }
                    write!(f, "}}")
                } else {
                    write!(f, "${{{name}}}")
                }
            }
            Self::Choice { id, choices } => {
                write!(f, "${{{id}|")?;
                for (i, choice) in choices.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{choice}")?;
                }
                write!(f, "|}}")
            }
        }
    }
}

impl fmt::Display for SnippetBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for elem in &self.0 {
            write!(f, "{elem}")?;
        }
        Ok(())
    }
}

impl fmt::Display for CaseModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Upcase => write!(f, "upcase"),
            Self::Downcase => write!(f, "downcase"),
            Self::Capitalize => write!(f, "capitalize"),
            Self::CamelCase => write!(f, "camelcase"),
            Self::PascalCase => write!(f, "pascalcase"),
            Self::SnakeCase => write!(f, "snakecase"),
            Self::KebabCase => write!(f, "kebabcase"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::literal_string_with_formatting_args)]
mod tests {
    use super::*;

    // =========================================================================
    // Test helpers — extract enum variants (coverage(off) for unreachable arms)
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn as_placeholder(elem: &SnippetElement) -> (TabStopId, &Vec<SnippetElement>) {
        match elem {
            SnippetElement::Placeholder { id, body } => (*id, body),
            other => panic!("expected Placeholder, got {other:?}"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn as_variable(
        elem: &SnippetElement,
    ) -> (&str, &Option<Vec<SnippetElement>>, &Option<Transform>) {
        match elem {
            SnippetElement::Variable {
                name,
                default,
                transform,
            } => (name, default, transform),
            other => panic!("expected Variable, got {other:?}"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn as_choice(elem: &SnippetElement) -> (TabStopId, &Vec<String>) {
        match elem {
            SnippetElement::Choice { id, choices } => (*id, choices),
            other => panic!("expected Choice, got {other:?}"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn as_tabstop(elem: &SnippetElement) -> (TabStopId, &Option<Transform>) {
        match elem {
            SnippetElement::TabStop { id, transform } => (*id, transform),
            other => panic!("expected TabStop, got {other:?}"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn as_conditional(item: &FormatItem) -> (usize, &str, &str) {
        match item {
            FormatItem::Conditional {
                capture,
                if_text,
                else_text,
            } => (*capture, if_text, else_text),
            other => panic!("expected Conditional, got {other:?}"),
        }
    }

    // =========================================================================
    // SnippetElement construction and equality
    // =========================================================================

    #[test]
    fn test_text_element() {
        let elem = SnippetElement::Text("hello".to_string());
        assert_eq!(elem, SnippetElement::Text("hello".to_string()));
    }

    #[test]
    fn test_tabstop_element() {
        let elem = SnippetElement::TabStop {
            id: 1,
            transform: None,
        };
        assert_eq!(
            elem,
            SnippetElement::TabStop {
                id: 1,
                transform: None,
            }
        );
    }

    #[test]
    fn test_placeholder_element() {
        let elem = SnippetElement::Placeholder {
            id: 1,
            body: vec![SnippetElement::Text("default".to_string())],
        };
        let (id, body) = as_placeholder(&elem);
        assert_eq!(id, 1);
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn test_variable_element() {
        let elem = SnippetElement::Variable {
            name: "TM_FILENAME".to_string(),
            default: None,
            transform: None,
        };
        let (name, default, transform) = as_variable(&elem);
        assert_eq!(name, "TM_FILENAME");
        assert!(default.is_none());
        assert!(transform.is_none());
    }

    #[test]
    fn test_variable_with_default() {
        let elem = SnippetElement::Variable {
            name: "VAR".to_string(),
            default: Some(vec![SnippetElement::Text("fallback".to_string())]),
            transform: None,
        };
        let (_, default, _) = as_variable(&elem);
        assert!(default.is_some());
        assert_eq!(default.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_choice_element() {
        let elem = SnippetElement::Choice {
            id: 1,
            choices: vec!["one".to_string(), "two".to_string(), "three".to_string()],
        };
        let (id, choices) = as_choice(&elem);
        assert_eq!(id, 1);
        assert_eq!(choices.len(), 3);
    }

    #[test]
    fn test_transform() {
        let t = Transform {
            regex: "(.*)".to_string(),
            replacement: vec![FormatItem::Text("$1".to_string())],
            options: "g".to_string(),
        };
        assert_eq!(t.regex, "(.*)");
        assert_eq!(t.options, "g");
    }

    #[test]
    fn test_format_item_text() {
        let item = FormatItem::Text("hello".to_string());
        assert_eq!(item, FormatItem::Text("hello".to_string()));
    }

    #[test]
    fn test_format_item_capture() {
        let item = FormatItem::Capture(1);
        assert_eq!(item, FormatItem::Capture(1));
    }

    #[test]
    fn test_format_item_case_change() {
        let item = FormatItem::CaseChange(1, CaseModifier::Upcase);
        assert_eq!(item, FormatItem::CaseChange(1, CaseModifier::Upcase));
    }

    #[test]
    fn test_format_item_conditional() {
        let item = FormatItem::Conditional {
            capture: 1,
            if_text: "yes".to_string(),
            else_text: "no".to_string(),
        };
        let (capture, if_text, else_text) = as_conditional(&item);
        assert_eq!(capture, 1);
        assert_eq!(if_text, "yes");
        assert_eq!(else_text, "no");
    }

    // =========================================================================
    // CaseModifier coverage
    // =========================================================================

    #[test]
    fn test_case_modifier_variants() {
        let modifiers = [
            CaseModifier::Upcase,
            CaseModifier::Downcase,
            CaseModifier::Capitalize,
            CaseModifier::CamelCase,
            CaseModifier::PascalCase,
            CaseModifier::SnakeCase,
            CaseModifier::KebabCase,
        ];
        // All variants are distinct
        for (i, a) in modifiers.iter().enumerate() {
            for (j, b) in modifiers.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn test_case_modifier_display() {
        assert_eq!(CaseModifier::Upcase.to_string(), "upcase");
        assert_eq!(CaseModifier::Downcase.to_string(), "downcase");
        assert_eq!(CaseModifier::Capitalize.to_string(), "capitalize");
        assert_eq!(CaseModifier::CamelCase.to_string(), "camelcase");
        assert_eq!(CaseModifier::PascalCase.to_string(), "pascalcase");
        assert_eq!(CaseModifier::SnakeCase.to_string(), "snakecase");
        assert_eq!(CaseModifier::KebabCase.to_string(), "kebabcase");
    }

    #[test]
    fn test_case_modifier_copy_clone() {
        let m = CaseModifier::Upcase;
        let cloned = m;
        assert_eq!(m, cloned);
    }

    // =========================================================================
    // SnippetBody
    // =========================================================================

    #[test]
    fn test_snippet_body_new() {
        let body = SnippetBody::new(vec![SnippetElement::Text("hello".to_string())]);
        assert_eq!(body.len(), 1);
        assert!(!body.is_empty());
    }

    #[test]
    fn test_snippet_body_empty() {
        let body = SnippetBody::new(vec![]);
        assert!(body.is_empty());
        assert_eq!(body.len(), 0);
    }

    #[test]
    fn test_snippet_body_elements() {
        let body = SnippetBody::new(vec![
            SnippetElement::Text("fn ".to_string()),
            SnippetElement::TabStop {
                id: 1,
                transform: None,
            },
        ]);
        assert_eq!(body.elements().len(), 2);
    }

    #[test]
    fn test_snippet_body_clone() {
        let body = SnippetBody::new(vec![SnippetElement::Text("x".to_string())]);
        let cloned = body.clone();
        assert_eq!(body, cloned);
    }

    // =========================================================================
    // Display implementations
    // =========================================================================

    #[test]
    fn test_display_text() {
        let elem = SnippetElement::Text("hello world".to_string());
        assert_eq!(elem.to_string(), "hello world");
    }

    #[test]
    fn test_display_tabstop() {
        let elem = SnippetElement::TabStop {
            id: 1,
            transform: None,
        };
        assert_eq!(elem.to_string(), "${1}");
    }

    #[test]
    fn test_display_tabstop_zero() {
        let elem = SnippetElement::TabStop {
            id: 0,
            transform: None,
        };
        assert_eq!(elem.to_string(), "${0}");
    }

    #[test]
    fn test_display_placeholder() {
        let elem = SnippetElement::Placeholder {
            id: 1,
            body: vec![SnippetElement::Text("name".to_string())],
        };
        assert_eq!(elem.to_string(), "${1:name}");
    }

    #[test]
    fn test_display_nested_placeholder() {
        let elem = SnippetElement::Placeholder {
            id: 1,
            body: vec![
                SnippetElement::Text("outer ".to_string()),
                SnippetElement::Placeholder {
                    id: 2,
                    body: vec![SnippetElement::Text("inner".to_string())],
                },
            ],
        };
        assert_eq!(elem.to_string(), "${1:outer ${2:inner}}");
    }

    #[test]
    fn test_display_variable_no_default() {
        let elem = SnippetElement::Variable {
            name: "TM_FILENAME".to_string(),
            default: None,
            transform: None,
        };
        assert_eq!(elem.to_string(), "${TM_FILENAME}");
    }

    #[test]
    fn test_display_variable_with_default() {
        let elem = SnippetElement::Variable {
            name: "VAR".to_string(),
            default: Some(vec![SnippetElement::Text("fallback".to_string())]),
            transform: None,
        };
        assert_eq!(elem.to_string(), "${VAR:fallback}");
    }

    #[test]
    fn test_display_choice() {
        let elem = SnippetElement::Choice {
            id: 1,
            choices: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        };
        assert_eq!(elem.to_string(), "${1|a,b,c|}");
    }

    #[test]
    fn test_display_choice_single() {
        let elem = SnippetElement::Choice {
            id: 2,
            choices: vec!["only".to_string()],
        };
        assert_eq!(elem.to_string(), "${2|only|}");
    }

    #[test]
    fn test_display_snippet_body() {
        let body = SnippetBody::new(vec![
            SnippetElement::Text("fn ".to_string()),
            SnippetElement::TabStop {
                id: 1,
                transform: None,
            },
            SnippetElement::Text("() {\n\t".to_string()),
            SnippetElement::TabStop {
                id: 0,
                transform: None,
            },
            SnippetElement::Text("\n}".to_string()),
        ]);
        assert_eq!(body.to_string(), "fn ${1}() {\n\t${0}\n}");
    }

    #[test]
    fn test_display_empty_body() {
        let body = SnippetBody::new(vec![]);
        assert_eq!(body.to_string(), "");
    }

    // =========================================================================
    // Clone / Debug coverage
    // =========================================================================

    #[test]
    fn test_element_clone() {
        let elem = SnippetElement::Placeholder {
            id: 1,
            body: vec![SnippetElement::Text("x".to_string())],
        };
        let cloned = elem.clone();
        assert_eq!(elem, cloned);
    }

    #[test]
    fn test_element_debug() {
        let elem = SnippetElement::Text("hi".to_string());
        let debug = format!("{elem:?}");
        assert!(debug.contains("Text"));
    }

    #[test]
    fn test_transform_clone_debug() {
        let t = Transform {
            regex: "x".to_string(),
            replacement: vec![],
            options: String::new(),
        };
        let cloned = t.clone();
        assert_eq!(t, cloned);
        let debug = format!("{t:?}");
        assert!(debug.contains("Transform"));
    }

    #[test]
    fn test_format_item_clone_debug() {
        let item = FormatItem::Text("x".to_string());
        let cloned = item.clone();
        assert_eq!(item, cloned);
        let debug = format!("{item:?}");
        assert!(debug.contains("Text"));
    }

    #[test]
    fn test_case_modifier_debug() {
        let m = CaseModifier::KebabCase;
        let debug = format!("{m:?}");
        assert!(debug.contains("KebabCase"));
    }

    #[test]
    fn test_tabstop_with_transform() {
        let t = Transform {
            regex: "(.+)".to_string(),
            replacement: vec![FormatItem::Capture(1)],
            options: String::new(),
        };
        let elem = SnippetElement::TabStop {
            id: 1,
            transform: Some(t.clone()),
        };
        let (id, transform) = as_tabstop(&elem);
        assert_eq!(id, 1);
        assert_eq!(transform.as_ref().unwrap(), &t);
    }

    #[test]
    fn test_variable_with_transform() {
        let t = Transform {
            regex: "(.+)".to_string(),
            replacement: vec![FormatItem::Text("replaced".to_string())],
            options: "g".to_string(),
        };
        let elem = SnippetElement::Variable {
            name: "VAR".to_string(),
            default: None,
            transform: Some(t),
        };
        let (_, _, transform) = as_variable(&elem);
        assert!(transform.is_some());
    }
}
