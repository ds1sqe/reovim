//! Scope context types for navigation and breadcrumbs.
//!
//! This module defines types for representing scope boundaries in source code,
//! such as function definitions, class/struct declarations, and module blocks.
//! Language modules provide scope queries; consumer modules (context, sticky-context)
//! use them for statusline breadcrumbs and viewport headers.

use std::fmt;

/// Kind of scope (what construct it represents).
///
/// Classifies scope boundaries by the type of syntax construct.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::ScopeKind;
///
/// let kind = ScopeKind::Function;
/// assert!(kind.is_definition());
/// assert_eq!(kind.as_str(), "fn");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// Function or method definition.
    Function,
    /// Class, struct, enum, trait, or impl block.
    Class,
    /// Module or namespace.
    Module,
    /// Generic block (if, for, while, match, etc.).
    Block,
    /// Heading (markdown, org-mode, etc.).
    Heading,
    /// Namespace (C++, etc.).
    Namespace,
}

impl ScopeKind {
    /// Check if this scope represents a definition (function, class, module, namespace).
    #[must_use]
    pub const fn is_definition(self) -> bool {
        matches!(self, Self::Function | Self::Class | Self::Module | Self::Namespace)
    }

    /// Get a short human-readable label for this scope kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "fn",
            Self::Class => "class",
            Self::Module => "mod",
            Self::Block => "block",
            Self::Heading => "heading",
            Self::Namespace => "namespace",
        }
    }
}

impl fmt::Display for ScopeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A scope boundary range in the buffer.
///
/// Represents a contiguous region of source code that defines a scope boundary,
/// identified by the syntax driver. Lines are 0-indexed.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::{ScopeRange, ScopeKind};
///
/// let scope = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", Some("main".to_string()));
/// assert!(scope.contains_line(10));
/// assert!(scope.is_multiline());
/// assert_eq!(scope.line_count(), 16);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeRange {
    /// Starting line (0-indexed).
    pub start_line: u32,
    /// Ending line (0-indexed, inclusive).
    pub end_line: u32,
    /// Kind of scope.
    pub kind: ScopeKind,
    /// Display text (e.g., "fn main", "impl Foo", "H2 Architecture").
    pub display_text: String,
    /// Just the identifier name (e.g., "main", "Foo"), if available.
    pub name: Option<String>,
}

impl ScopeRange {
    /// Create a new scope range.
    #[must_use]
    pub fn new(
        start_line: u32,
        end_line: u32,
        kind: ScopeKind,
        display_text: impl Into<String>,
        name: Option<String>,
    ) -> Self {
        Self {
            start_line,
            end_line,
            kind,
            display_text: display_text.into(),
            name,
        }
    }

    /// Check if this scope contains a line.
    #[must_use]
    pub const fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// Get the number of lines in this scope.
    #[must_use]
    pub const fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Check if this scope spans multiple lines.
    #[must_use]
    pub const fn is_multiline(&self) -> bool {
        self.end_line > self.start_line
    }
}

/// Hierarchy of enclosing scopes at a cursor position.
///
/// Items are ordered outermost-first (module -> class -> function).
/// Used by consumer modules to build statusline breadcrumbs and
/// sticky-context viewport headers.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::{ContextHierarchy, ScopeRange, ScopeKind};
///
/// let scopes = vec![
///     ScopeRange::new(0, 100, ScopeKind::Module, "mod utils", Some("utils".to_string())),
///     ScopeRange::new(5, 20, ScopeKind::Function, "fn main", Some("main".to_string())),
/// ];
/// let ctx = ContextHierarchy::new(0, 10, 5, scopes);
///
/// assert_eq!(ctx.len(), 2);
/// assert_eq!(ctx.to_breadcrumb(" > "), "mod utils > fn main");
/// ```
#[derive(Debug, Clone, Default)]
pub struct ContextHierarchy {
    /// Scopes from outermost to innermost.
    pub items: Vec<ScopeRange>,
    /// Buffer identifier (set by the module layer, not the driver).
    pub buffer_id: usize,
    /// Line where context was computed (0-indexed).
    pub line: u32,
    /// Column where context was computed (0-indexed).
    pub col: u32,
}

impl ContextHierarchy {
    /// Create a new context hierarchy.
    #[must_use]
    pub const fn new(buffer_id: usize, line: u32, col: u32, items: Vec<ScopeRange>) -> Self {
        Self {
            items,
            buffer_id,
            line,
            col,
        }
    }

    /// Create an empty context hierarchy.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            items: Vec::new(),
            buffer_id: 0,
            line: 0,
            col: 0,
        }
    }

    /// Check if the hierarchy is empty (no enclosing scopes).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of enclosing scopes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    /// Get the innermost (current) scope.
    #[must_use]
    pub fn current_scope(&self) -> Option<&ScopeRange> {
        self.items.last()
    }

    /// Format as a breadcrumb string with the given separator.
    ///
    /// Example: `"mod utils > fn main > impl Foo"`
    #[must_use]
    pub fn to_breadcrumb(&self, separator: &str) -> String {
        self.items
            .iter()
            .map(|s| s.display_text.as_str())
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// Format as a breadcrumb string with a maximum number of items.
    ///
    /// When there are more items than `max`, the outermost items are
    /// dropped and replaced with "...".
    ///
    /// # Example
    ///
    /// With 5 scopes and max=3: `"... > class Foo > fn bar"`
    #[must_use]
    pub fn to_breadcrumb_max(&self, separator: &str, max: usize) -> String {
        if self.items.len() <= max {
            return self.to_breadcrumb(separator);
        }

        let skip = self.items.len() - max;
        let mut parts = vec!["..."];
        parts.extend(self.items[skip..].iter().map(|s| s.display_text.as_str()));
        parts.join(separator)
    }
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
