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
#[path = "ast_tests.rs"]
mod tests;
