//! Core annotation types for the gutter annotation system.
//!
//! Annotations are pieces of information attached to locations in a buffer,
//! displayed in the gutter area. Examples include line numbers, diagnostics,
//! git diff indicators, and fold markers.
//!
//! # Architecture
//!
//! The annotation system follows mechanism/policy separation:
//! - **Mechanism** (this module): Data types and contracts
//! - **Policy** (modules): Specific implementations (e.g., `LineNumberSource`)
//!
//! # Example
//!
//! ```
//! use reovim_driver_annotation::{
//!     Annotation, AnnotationKind, AnnotationTarget, AnnotationPayload,
//! };
//!
//! // Create a line number annotation
//! let annotation = Annotation {
//!     kind: AnnotationKind::new("line_number"),
//!     target: AnnotationTarget::Line(0),
//!     priority: 0,
//!     payload: AnnotationPayload::Number(1),
//! };
//!
//! // Create a diagnostic annotation with namespace
//! let diagnostic = Annotation {
//!     kind: AnnotationKind::new("diagnostic.error"),
//!     target: AnnotationTarget::Line(10),
//!     priority: 50,
//!     payload: AnnotationPayload::Severity(0), // 0 = error
//! };
//!
//! assert_eq!(diagnostic.kind.namespace(), Some("diagnostic"));
//! ```

use std::sync::Arc;

/// Hierarchical identifier for annotation kinds.
///
/// Annotation kinds use a dot-separated namespace convention:
/// - `"line_number"` - No namespace (top-level)
/// - `"diagnostic.error"` - Namespace "diagnostic", name "error"
/// - `"git.add"` - Namespace "git", name "add"
///
/// This enables pattern matching in presenters (e.g., `"diagnostic.*"` matches
/// all diagnostic kinds).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnnotationKind(Arc<str>);

impl AnnotationKind {
    /// Create a new annotation kind.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_annotation::AnnotationKind;
    ///
    /// let kind = AnnotationKind::new("diagnostic.error");
    /// assert_eq!(kind.name(), "diagnostic.error");
    /// assert_eq!(kind.namespace(), Some("diagnostic"));
    /// ```
    #[must_use]
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self(name.into())
    }

    /// Get the full name of this annotation kind.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }

    /// Get the namespace prefix, if any.
    ///
    /// Returns the part before the first dot, or `None` if there's no dot.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_annotation::AnnotationKind;
    ///
    /// let namespaced = AnnotationKind::new("diagnostic.error");
    /// assert_eq!(namespaced.namespace(), Some("diagnostic"));
    ///
    /// let top_level = AnnotationKind::new("line_number");
    /// assert_eq!(top_level.namespace(), None);
    /// ```
    #[must_use]
    pub fn namespace(&self) -> Option<&str> {
        self.0.find('.').map(|idx| &self.0[..idx])
    }

    /// Check if this kind starts with the given prefix.
    ///
    /// Used for pattern matching in presenter lookup.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_annotation::AnnotationKind;
    ///
    /// let kind = AnnotationKind::new("diagnostic.error");
    /// assert!(kind.is_prefix("diagnostic"));
    /// assert!(!kind.is_prefix("git"));
    /// ```
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[must_use]
    pub fn is_prefix(&self, prefix: &str) -> bool {
        if self.0.len() < prefix.len() {
            return false;
        }
        // Must match prefix exactly and be followed by '.' or end of string
        if !self.0.starts_with(prefix) {
            return false;
        }
        // Check that it's a proper prefix boundary (followed by '.' or exact match)
        self.0.len() == prefix.len() || self.0.as_bytes().get(prefix.len()) == Some(&b'.')
    }
}

impl std::fmt::Display for AnnotationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Where an annotation applies in the buffer.
///
/// Annotations can target:
/// - A single line
/// - A range of lines (inclusive)
/// - A specific point (line + column)
/// - The entire buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationTarget {
    /// Single line (0-indexed).
    Line(usize),

    /// Line range, inclusive of both start and end (0-indexed).
    Range {
        /// Starting line (inclusive)
        start: usize,
        /// Ending line (inclusive)
        end: usize,
    },

    /// Specific point in buffer (line + column, both 0-indexed).
    Point {
        /// Line number
        line: usize,
        /// Column (byte offset)
        column: usize,
    },

    /// Entire buffer (e.g., for buffer-wide indicators).
    Buffer,
}

impl AnnotationTarget {
    /// Create a range target.
    #[must_use]
    pub const fn range(start: usize, end: usize) -> Self {
        Self::Range { start, end }
    }

    /// Create a point target.
    #[must_use]
    pub const fn point(line: usize, column: usize) -> Self {
        Self::Point { line, column }
    }

    /// Check if this target affects the given line.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_annotation::AnnotationTarget;
    ///
    /// let line = AnnotationTarget::Line(5);
    /// assert!(line.affects_line(5));
    /// assert!(!line.affects_line(4));
    ///
    /// let range = AnnotationTarget::range(5, 10);
    /// assert!(range.affects_line(5));
    /// assert!(range.affects_line(7));
    /// assert!(range.affects_line(10));
    /// assert!(!range.affects_line(11));
    ///
    /// let buffer = AnnotationTarget::Buffer;
    /// assert!(buffer.affects_line(0));
    /// assert!(buffer.affects_line(1000));
    /// ```
    #[must_use]
    pub const fn affects_line(&self, line: usize) -> bool {
        match *self {
            Self::Line(l) | Self::Point { line: l, .. } => l == line,
            Self::Range { start, end } => line >= start && line <= end,
            Self::Buffer => true,
        }
    }

    /// Get the starting line of this target.
    ///
    /// Returns 0 for `Buffer` targets.
    #[must_use]
    pub const fn start_line(&self) -> usize {
        match *self {
            Self::Line(l) | Self::Point { line: l, .. } => l,
            Self::Range { start, .. } => start,
            Self::Buffer => 0,
        }
    }

    /// Get the ending line of this target.
    ///
    /// Returns `usize::MAX` for `Buffer` targets.
    #[must_use]
    pub const fn end_line(&self) -> usize {
        match *self {
            Self::Line(l) | Self::Point { line: l, .. } => l,
            Self::Range { end, .. } => end,
            Self::Buffer => usize::MAX,
        }
    }

    /// Check if this is a single-line target.
    #[must_use]
    pub const fn is_single_line(&self) -> bool {
        matches!(self, Self::Line(_) | Self::Point { .. })
    }
}

/// Kind-specific data carried by an annotation.
///
/// Different annotation types need different data:
/// - Line numbers need a numeric value
/// - Diagnostics need a severity level
/// - Fold markers need an open/closed state
/// - Some annotations need no additional data
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AnnotationPayload {
    /// Numeric value (line numbers, counts).
    Number(usize),

    /// Text label or message.
    Text(String),

    /// Severity level (0=error, 1=warn, 2=info, 3=hint).
    Severity(u8),

    /// Boolean state (folded, enabled, etc.).
    State(bool),

    /// No additional data needed.
    #[default]
    None,
}

impl AnnotationPayload {
    /// Create a text payload.
    #[must_use]
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// Check if this payload has no data.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Get the numeric value, if this is a `Number` payload.
    #[must_use]
    pub const fn as_number(&self) -> Option<usize> {
        match *self {
            Self::Number(n) => Some(n),
            _ => None,
        }
    }

    /// Get the text value, if this is a `Text` payload.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Get the severity level, if this is a `Severity` payload.
    #[must_use]
    pub const fn as_severity(&self) -> Option<u8> {
        match *self {
            Self::Severity(s) => Some(s),
            _ => None,
        }
    }

    /// Get the state value, if this is a `State` payload.
    #[must_use]
    pub const fn as_state(&self) -> Option<bool> {
        match *self {
            Self::State(s) => Some(s),
            _ => None,
        }
    }
}

/// A piece of information attached to a location in a buffer.
///
/// Annotations are the core data type of the gutter annotation system.
/// They carry:
/// - **Kind**: What type of annotation (line number, diagnostic, etc.)
/// - **Target**: Where it applies (line, range, point, buffer)
/// - **Priority**: For conflict resolution when multiple annotations compete
/// - **Payload**: Kind-specific data
///
/// # Priority
///
/// When multiple annotations compete for the same gutter position,
/// higher priority values win. Typical priorities:
/// - 0: Line numbers (lowest, always present)
/// - 10: Git indicators
/// - 20: Fold markers
/// - 30-50: Diagnostics (higher severity = higher priority)
/// - 100+: User signs (highest)
///
/// # Example
///
/// ```
/// use reovim_driver_annotation::{
///     Annotation, AnnotationKind, AnnotationTarget, AnnotationPayload,
/// };
///
/// let annotation = Annotation {
///     kind: AnnotationKind::new("line_number"),
///     target: AnnotationTarget::Line(0),
///     priority: 0,
///     payload: AnnotationPayload::Number(1),
/// };
///
/// assert!(annotation.affects_line(0));
/// assert!(!annotation.affects_line(1));
/// ```
#[derive(Debug, Clone)]
pub struct Annotation {
    /// What kind of annotation this is.
    pub kind: AnnotationKind,

    /// Where this annotation applies.
    pub target: AnnotationTarget,

    /// Priority for conflict resolution (higher wins).
    pub priority: u8,

    /// Kind-specific data.
    pub payload: AnnotationPayload,
}

impl Annotation {
    /// Create a new annotation.
    #[must_use]
    pub const fn new(
        kind: AnnotationKind,
        target: AnnotationTarget,
        priority: u8,
        payload: AnnotationPayload,
    ) -> Self {
        Self {
            kind,
            target,
            priority,
            payload,
        }
    }

    /// Create a line number annotation.
    ///
    /// Convenience constructor for the most common annotation type.
    #[must_use]
    pub fn line_number(line: usize, number: usize) -> Self {
        Self {
            kind: AnnotationKind::new("line_number"),
            target: AnnotationTarget::Line(line),
            priority: 0,
            payload: AnnotationPayload::Number(number),
        }
    }

    /// Check if this annotation affects the given line.
    #[must_use]
    pub const fn affects_line(&self, line: usize) -> bool {
        self.target.affects_line(line)
    }

    /// Get the starting line of this annotation.
    #[must_use]
    pub const fn start_line(&self) -> usize {
        self.target.start_line()
    }

    /// Get the ending line of this annotation.
    #[must_use]
    pub const fn end_line(&self) -> usize {
        self.target.end_line()
    }
}

// Ensure types are Send + Sync for async compatibility
const _: () = {
    #[cfg_attr(coverage_nightly, coverage(off))]
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AnnotationKind>();
    assert_send_sync::<AnnotationTarget>();
    assert_send_sync::<AnnotationPayload>();
    assert_send_sync::<Annotation>();
};

#[cfg(test)]
#[path = "annotation_tests.rs"]
mod tests;
