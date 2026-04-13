//! Annotation source trait for extensible annotation providers.
//!
//! Modules implement [`AnnotationSource`] to supply annotations for the gutter.
//! Sources are responsible for generating annotations (the data), while
//! presenters handle the visual rendering.
//!
//! # Architecture
//!
//! ```text
//! AnnotationSource (mechanism trait - this module)
//!        |  implements
//! LineNumberSource (policy - in server/modules/vim)
//! DiagnosticSource (policy - in server/modules/lsp)
//! GitDiffSource    (policy - in server/modules/git)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_subsys_annotation::{
//!     Annotation, AnnotationContext, AnnotationKind, AnnotationSource,
//!     AnnotationTarget, AnnotationPayload,
//! };
//! use reovim_kernel::api::v1::BufferId;
//! use std::ops::Range;
//!
//! struct BookmarkSource {
//!     bookmarks: Vec<usize>, // Line numbers with bookmarks
//! }
//!
//! impl AnnotationSource for BookmarkSource {
//!     fn id(&self) -> &'static str {
//!         "feature.bookmark"
//!     }
//!
//!     fn provides(&self) -> Vec<AnnotationKind> {
//!         vec![AnnotationKind::new("bookmark")]
//!     }
//!
//!     fn annotations(
//!         &self,
//!         _buffer_id: BufferId,
//!         range: Range<usize>,
//!         _context: &AnnotationContext,
//!     ) -> Vec<Annotation> {
//!         self.bookmarks
//!             .iter()
//!             .filter(|&&line| range.contains(&line))
//!             .map(|&line| Annotation {
//!                 kind: AnnotationKind::new("bookmark"),
//!                 target: AnnotationTarget::Line(line),
//!                 priority: 20,
//!                 payload: AnnotationPayload::None,
//!             })
//!             .collect()
//!     }
//! }
//! ```

use {
    super::types::{Annotation, AnnotationKind},
    crate::LineNumberMode,
    reovim_kernel::api::v1::BufferId,
    std::{ops::Range, path::PathBuf},
};

/// Context available to annotation sources when generating annotations.
///
/// This provides information about the buffer and editing state that
/// sources may need to generate context-aware annotations.
///
/// # Example
///
/// ```
/// use reovim_subsys_annotation::AnnotationContext;
///
/// let context = AnnotationContext::new(100, 50, "NORMAL");
///
/// assert_eq!(context.total_lines, 100);
/// assert_eq!(context.cursor_line, 50);
/// ```
#[derive(Debug, Clone, Default)]
pub struct AnnotationContext {
    /// Total number of lines in the buffer.
    pub total_lines: usize,

    /// Current cursor line (0-indexed).
    pub cursor_line: usize,

    /// Current editor mode (e.g., "NORMAL", "INSERT").
    pub mode: String,

    /// Line number display mode (set by caller based on options).
    ///
    /// When `Some`, sources should use this mode for line number display.
    /// When `None`, sources use their default/stored mode.
    pub line_number_mode: Option<LineNumberMode>,

    /// File path associated with the buffer, if known.
    ///
    /// Used by sources that need filesystem context (e.g., git signs,
    /// git blame) to map buffer content to file-level operations.
    pub file_path: Option<PathBuf>,
}

impl AnnotationContext {
    /// Create a new annotation context.
    #[must_use]
    pub fn new(total_lines: usize, cursor_line: usize, mode: impl Into<String>) -> Self {
        Self {
            total_lines,
            cursor_line,
            mode: mode.into(),
            line_number_mode: None,
            file_path: None,
        }
    }

    /// Create a context with line number mode specified.
    #[must_use]
    pub fn with_line_number_mode(
        total_lines: usize,
        cursor_line: usize,
        mode: impl Into<String>,
        line_number_mode: LineNumberMode,
    ) -> Self {
        Self {
            total_lines,
            cursor_line,
            mode: mode.into(),
            line_number_mode: Some(line_number_mode),
            file_path: None,
        }
    }

    /// Create a minimal context for testing.
    ///
    /// Note: Cannot be const because `String::new()` is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn minimal(total_lines: usize) -> Self {
        Self {
            total_lines,
            cursor_line: 0,
            mode: String::new(),
            line_number_mode: None,
            file_path: None,
        }
    }

    /// Get the line number mode if set.
    #[must_use]
    pub const fn line_number_mode(&self) -> Option<LineNumberMode> {
        self.line_number_mode
    }
}

/// Trait for annotation sources (mechanism).
///
/// Modules implement this trait to provide annotation data for the gutter.
/// Each source is responsible for:
/// - Declaring what annotation kinds it provides
/// - Generating annotations for requested line ranges
///
/// # Design
///
/// Sources are part of the mechanism layer - they define *what* annotations
/// exist, not *how* they look. Presentation is handled by `AnnotationPresenter`
/// in the display driver.
///
/// # Thread Safety
///
/// Sources must be `Send + Sync` to support concurrent access from
/// multiple clients attached to the same session.
///
/// # Concurrency Model
///
/// The annotation system uses a single-threaded model where all source
/// queries run on the runner event loop. For expensive operations (like
/// LSP queries), sources should:
/// 1. Return cached results immediately from `annotations()`
/// 2. Spawn background tasks to fetch fresh data
/// 3. Update internal cache when data arrives
/// 4. Signal that annotations changed (triggering re-render)
pub trait AnnotationSource: Send + Sync {
    /// Unique identifier for this source.
    ///
    /// Used for:
    /// - Logging and debugging
    /// - Source isolation in storage
    /// - Configuration targeting
    ///
    /// Convention: Use reverse-domain style (e.g., `"builtin.line_number"`,
    /// `"plugin.git"`, `"feature.bookmark"`).
    fn id(&self) -> &'static str;

    /// Annotation kinds this source produces.
    ///
    /// This declaration enables:
    /// - Presenter lookup optimization
    /// - Configuration filtering
    /// - Documentation generation
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn provides(&self) -> Vec<AnnotationKind> {
    ///     vec![
    ///         AnnotationKind::new("diagnostic.error"),
    ///         AnnotationKind::new("diagnostic.warning"),
    ///         AnnotationKind::new("diagnostic.info"),
    ///         AnnotationKind::new("diagnostic.hint"),
    ///     ]
    /// }
    /// ```
    fn provides(&self) -> Vec<AnnotationKind>;

    /// Get annotations for a buffer region.
    ///
    /// # Parameters
    ///
    /// - `buffer_id`: The buffer to query annotations for
    /// - `range`: Line range to query (exclusive end, like `0..10` for lines 0-9)
    /// - `context`: Additional context about the buffer state
    ///
    /// # Returns
    ///
    /// A vector of annotations affecting lines in the given range.
    /// Annotations outside the range should be filtered out.
    ///
    /// # Performance
    ///
    /// This method is called frequently during rendering. Implementations
    /// should be efficient:
    /// - Use indexed data structures for range queries
    /// - Cache expensive computations
    /// - Return empty vec quickly when no annotations exist
    fn annotations(
        &self,
        buffer_id: BufferId,
        range: Range<usize>,
        context: &AnnotationContext,
    ) -> Vec<Annotation>;

    /// Called when buffer content changes (optional).
    ///
    /// Sources can use this to invalidate caches or trigger background
    /// updates. The default implementation does nothing.
    ///
    /// # Parameters
    ///
    /// - `buffer_id`: The buffer that changed
    ///
    /// # Note
    ///
    /// This is a notification, not a request. The source should update
    /// its internal state but not block on expensive operations.
    #[allow(unused_variables)]
    fn on_buffer_change(&self, buffer_id: BufferId) {
        // Default: do nothing
    }

    /// Check if the source has annotations for a buffer.
    ///
    /// Used for optimization - if a source has no annotations for a buffer,
    /// we can skip querying it entirely.
    ///
    /// The default implementation returns `true` (assume annotations exist).
    /// Sources with expensive queries should override this.
    #[allow(unused_variables)]
    fn has_annotations(&self, buffer_id: BufferId) -> bool {
        true
    }
}

// Ensure trait is object-safe by checking we can create trait objects
const _: () = {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _assert_object_safe(_: &dyn AnnotationSource) {}
};

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
