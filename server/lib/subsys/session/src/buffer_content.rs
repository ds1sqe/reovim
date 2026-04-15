//! Domain-neutral buffer content access.
//!
//! [`BufferContentProvider`] is a service trait for domain-neutral content
//! access. The server uses it for gRPC rendering (display lines) and
//! transport (save to disk, binary content).
//!
//! # Why Service Trait, Not Codec
//!
//! Buffer content differs fundamentally from Position/Cursor:
//! - **Large**: megabytes. Header+content doesn't scale.
//! - **Rendered**: server needs display lines, not raw domain data.
//! - **Stateful**: content lives in a registry, not passed as values.
//! - **Streaming**: saving to disk needs streaming, not in-memory bytes.
//!
//! # Implementations
//!
//! - Text domain: rope + syntax highlighting → lines of highlighted text
//! - Mesh domain: 3D viewport rendering → projection info
//! - Image domain: pixel info → color values per region

use reovim_kernel::api::v1::BufferId;

/// Domain-neutral buffer content access.
///
/// Each domain driver provides a single `Arc<dyn BufferContentProvider>` that
/// the server holds for async-safe access across `.await` boundaries.
///
/// # Object Safety
///
/// This trait is object-safe. The server stores `Arc<dyn BufferContentProvider>`.
pub trait BufferContentProvider: Send + Sync {
    /// Raw content bytes (for save-to-disk, gRPC binary content).
    ///
    /// Returns `None` if the buffer doesn't exist in this domain.
    fn content_bytes(&self, buffer_id: BufferId) -> Option<Vec<u8>>;

    /// Total size in bytes.
    ///
    /// Returns `None` if the buffer doesn't exist in this domain.
    fn content_size(&self, buffer_id: BufferId) -> Option<u64>;

    /// Domain-specific unit count.
    ///
    /// For text: line count. For mesh: object count. For image: layer count.
    /// Returns `None` if the buffer doesn't exist or the concept doesn't apply.
    fn content_unit_count(&self, buffer_id: BufferId) -> Option<usize>;

    /// Display lines for a region.
    ///
    /// `offset` is the first unit to render (0-indexed).
    /// `count` is the maximum number of units to return.
    ///
    /// Domain decides how to render:
    /// - Text shows source lines with syntax highlighting
    /// - Mesh shows viewport projection info
    /// - Image shows pixel/color info
    ///
    /// Returns `None` if the buffer doesn't exist in this domain.
    fn display_lines(
        &self,
        buffer_id: BufferId,
        offset: usize,
        count: usize,
    ) -> Option<Vec<DisplayLine>>;

    /// Whether the buffer has been modified since last save.
    fn is_modified(&self, buffer_id: BufferId) -> bool;

    /// Write buffer content to a writer (for saving to disk).
    ///
    /// Uses `dyn Write` (not generic `W: Write`) to maintain object safety.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to `writer` fails (I/O error from the
    /// underlying writer, e.g. disk full or broken pipe).
    fn write_to(&self, buffer_id: BufferId, writer: &mut dyn std::io::Write)
    -> std::io::Result<()>;
}

/// A single display line for gRPC rendering.
///
/// Domain-specific rendering produces these. Text domain fills `content`
/// with the source line text. Other domains fill it with their rendering.
#[derive(Debug, Clone)]
pub struct DisplayLine {
    /// The rendered content of this line.
    pub content: String,
    /// The domain-specific unit index (text: line number, mesh: object index).
    pub unit_index: usize,
}
