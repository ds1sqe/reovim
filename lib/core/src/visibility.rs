//! Visibility provider traits for rendering
//!
//! This module defines abstract traits for determining what content is visible.
//! The design is position-agnostic, allowing different visibility concepts:
//! - Line visibility (folds)
//! - Cell visibility (future: horizontal folding)
//! - Range visibility (future: text object hiding)

/// Query for visibility checks
///
/// Represents different types of position queries that can be made
/// to a visibility provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityQuery {
    /// Single line query
    Line(u32),
    /// Single cell query (line, column)
    Cell { line: u32, col: u32 },
    /// Line range query (inclusive)
    LineRange { start: u32, end: u32 },
}

impl From<u32> for VisibilityQuery {
    fn from(line: u32) -> Self {
        Self::Line(line)
    }
}

impl From<(u32, u32)> for VisibilityQuery {
    fn from((line, col): (u32, u32)) -> Self {
        Self::Cell { line, col }
    }
}

/// Marker displayed when content is hidden
///
/// Used by renderers to show indicators for collapsed/hidden content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibilityMarker {
    /// Number of hidden units (lines, columns, etc.)
    pub hidden_count: u32,
    /// Preview text to display
    pub preview: String,
}

impl VisibilityMarker {
    /// Create a new visibility marker
    #[must_use]
    pub fn new(hidden_count: u32, preview: impl Into<String>) -> Self {
        Self {
            hidden_count,
            preview: preview.into(),
        }
    }
}

/// Abstract visibility provider trait
///
/// This trait abstracts visibility logic from rendering, allowing plugins to
/// control what content is visible without tight coupling to core types.
///
/// Implementations only need to handle the query types they support,
/// returning `false` for `is_hidden` and `None` for `get_marker` for
/// unsupported query types.
///
/// # Example
///
/// ```ignore
/// struct FoldVisibility { /* ... */ }
///
/// impl VisibilityProvider for FoldVisibility {
///     fn is_hidden(&self, query: VisibilityQuery) -> bool {
///         match query {
///             VisibilityQuery::Line(line) => self.is_line_hidden(line),
///             _ => false, // Only handles line queries
///         }
///     }
///
///     fn get_marker(&self, query: VisibilityQuery) -> Option<VisibilityMarker> {
///         match query {
///             VisibilityQuery::Line(line) => self.get_fold_marker(line),
///             _ => None,
///         }
///     }
/// }
/// ```
pub trait VisibilityProvider: Send + Sync {
    /// Check if content at the queried position is hidden
    ///
    /// Returns `true` if the content should not be rendered.
    fn is_hidden(&self, query: VisibilityQuery) -> bool;

    /// Get visibility marker for the queried position
    ///
    /// Returns marker info if the position starts a hidden region.
    fn get_marker(&self, query: VisibilityQuery) -> Option<VisibilityMarker>;
}

/// No-op visibility provider that shows everything
///
/// Used as default when no visibility provider is available.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpVisibility;

impl VisibilityProvider for NoOpVisibility {
    fn is_hidden(&self, _query: VisibilityQuery) -> bool {
        false
    }

    fn get_marker(&self, _query: VisibilityQuery) -> Option<VisibilityMarker> {
        None
    }
}

/// Static no-op visibility provider instance
pub static NO_OP_VISIBILITY: NoOpVisibility = NoOpVisibility;

/// Provides visibility for multiple buffers
///
/// This trait allows plugins to provide per-buffer visibility without
/// core needing to know about concrete types like `FoldManager`.
///
/// Methods take `buffer_id` and `query` directly, avoiding lifetime issues
/// that would arise from returning trait object references.
pub trait BufferVisibilitySource: Send + Sync {
    /// Check if content at the queried position is hidden in a buffer
    fn is_hidden(&self, buffer_id: usize, query: VisibilityQuery) -> bool;

    /// Get visibility marker for the queried position in a buffer
    fn get_marker(&self, buffer_id: usize, query: VisibilityQuery) -> Option<VisibilityMarker>;
}

/// No-op buffer visibility source that returns no-op for all buffers
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpBufferVisibility;

impl BufferVisibilitySource for NoOpBufferVisibility {
    fn is_hidden(&self, _buffer_id: usize, _query: VisibilityQuery) -> bool {
        false
    }

    fn get_marker(&self, _buffer_id: usize, _query: VisibilityQuery) -> Option<VisibilityMarker> {
        None
    }
}

/// Static no-op buffer visibility source
pub static NO_OP_BUFFER_VISIBILITY: NoOpBufferVisibility = NoOpBufferVisibility;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visibility_query_from() {
        let line_query: VisibilityQuery = 5u32.into();
        assert_eq!(line_query, VisibilityQuery::Line(5));

        let cell_query: VisibilityQuery = (10u32, 20u32).into();
        assert_eq!(cell_query, VisibilityQuery::Cell { line: 10, col: 20 });
    }

    #[test]
    fn test_visibility_marker() {
        let marker = VisibilityMarker::new(5, "...");
        assert_eq!(marker.hidden_count, 5);
        assert_eq!(marker.preview, "...");
    }

    #[test]
    fn test_no_op_visibility() {
        let provider = NoOpVisibility;
        assert!(!provider.is_hidden(VisibilityQuery::Line(0)));
        assert!(!provider.is_hidden(VisibilityQuery::Line(100)));
        assert!(!provider.is_hidden(VisibilityQuery::Cell { line: 0, col: 0 }));
        assert!(provider.get_marker(VisibilityQuery::Line(0)).is_none());
    }

    #[test]
    fn test_custom_provider() {
        struct TestProvider;
        impl VisibilityProvider for TestProvider {
            fn is_hidden(&self, query: VisibilityQuery) -> bool {
                match query {
                    VisibilityQuery::Line(line) => line == 5 || line == 6,
                    _ => false,
                }
            }
            fn get_marker(&self, query: VisibilityQuery) -> Option<VisibilityMarker> {
                match query {
                    VisibilityQuery::Line(5) => Some(VisibilityMarker::new(2, "...")),
                    _ => None,
                }
            }
        }

        let provider = TestProvider;
        assert!(!provider.is_hidden(VisibilityQuery::Line(4)));
        assert!(provider.is_hidden(VisibilityQuery::Line(5)));
        assert!(provider.is_hidden(VisibilityQuery::Line(6)));
        assert!(!provider.is_hidden(VisibilityQuery::Line(7)));

        assert!(provider.get_marker(VisibilityQuery::Line(5)).is_some());
        assert!(provider.get_marker(VisibilityQuery::Line(6)).is_none());
    }
}
