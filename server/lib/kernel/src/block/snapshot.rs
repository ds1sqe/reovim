//! Buffer state capture and restore.
//!
//! Snapshots capture the complete state of a buffer at a point in time,
//! enabling restore operations for recovery or checkpointing.
//!
//! # Cursor Isolation (#471)
//!
//! Cursor position is passed explicitly to `capture()` and returned from
//! `restore()`. This is because cursor is now per-client state in Window,
//! not Buffer. The caller (session layer) is responsible for getting cursor
//! from Window before capture and setting it back to Window after restore.
//!
//! # Structural Sharing (#711)
//!
//! Snapshots store a rope clone, which is O(1) via `Arc` sharing.
//! The snapshot shares unchanged text nodes with the original buffer.
//!
//! # Design Principle
//!
//! Following the kernel purity principle, this module provides pure Rust
//! data structures and accessors. File serialization (JSON, etc.) is
//! handled by the driver layer (`server/lib/drivers/vfs/`).

use {
    crate::mm::{Buffer, BufferId, Position, Rope, VirtualBuffer, VirtualSnapshot},
    std::time::SystemTime,
};

/// Internal discriminant for snapshot content.
///
/// `Snapshot` stores text as either a Rope clone (small file) or a
/// `VirtualSnapshot` (large file backed by mmap + piece table).
/// This is never exposed to callers — they use `Snapshot` methods.
#[derive(Clone)]
enum SnapshotContent {
    /// Rope-based snapshot (existing small-file path).
    Rope(Rope),
    /// Piece-table snapshot for mmap-backed large files.
    Virtual(VirtualSnapshot),
}

/// A captured buffer state.
///
/// Snapshots are used for:
/// - Safety checkpoints before complex operations
/// - Crash recovery
/// - Cursor restoration when reopening files
///
/// # Cursor Isolation (#471)
///
/// Cursor is passed explicitly to `capture()` and returned from `restore()`.
/// The caller manages cursor via Window, not Buffer.
///
/// # File Storage
///
/// This struct provides accessors for all data needed by the driver
/// layer to serialize/deserialize to files. The kernel does not handle
/// serialization directly.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// // Create a buffer with some content
/// let mut buffer = Buffer::from_string("Hello, World!");
/// let cursor = Position::new(0, 0); // Get from Window in real usage
///
/// // Capture state with explicit cursor
/// let snapshot = Snapshot::capture(&buffer, cursor);
///
/// // Later, restore the state
/// let restored_cursor = snapshot.restore(&mut buffer);
/// // Caller sets restored_cursor to Window
/// ```
#[derive(Clone)]
pub struct Snapshot {
    /// Text content at time of capture.
    content: SnapshotContent,
    /// Cursor position at time of capture.
    cursor: Position,
    /// Buffer ID (for validation on restore).
    buffer_id: BufferId,
    /// When snapshot was taken.
    timestamp: SystemTime,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content_desc = match &self.content {
            SnapshotContent::Rope(rope) => format!("Rope({} lines)", rope.line_count()),
            SnapshotContent::Virtual(vs) => format!("{vs:?}"),
        };
        f.debug_struct("Snapshot")
            .field("content", &content_desc)
            .field("cursor", &self.cursor)
            .field("buffer_id", &self.buffer_id)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

impl Snapshot {
    /// Capture the current state of a Rope-based buffer.
    ///
    /// Cursor position must be passed explicitly - get it from Window.
    /// This is O(1) — the rope is cloned via `Arc` sharing.
    #[must_use]
    pub fn capture(buffer: &Buffer, cursor: Position) -> Self {
        Self {
            content: SnapshotContent::Rope(buffer.clone_rope()),
            cursor,
            buffer_id: buffer.id(),
            timestamp: SystemTime::now(),
        }
    }

    /// Capture the current state of a `VirtualBuffer` (mmap + piece table).
    ///
    /// This is O(1) — the `PieceTree` is cloned via `Arc` structural sharing.
    #[must_use]
    pub fn capture_virtual(vbuf: &VirtualBuffer, cursor: Position) -> Self {
        Self {
            content: SnapshotContent::Virtual(vbuf.capture_snapshot()),
            cursor,
            buffer_id: vbuf.id(),
            timestamp: SystemTime::now(),
        }
    }

    /// Create a snapshot from components.
    ///
    /// This is useful for restoring from serialized data.
    #[must_use]
    pub fn from_parts(
        lines: &[String],
        cursor: Position,
        buffer_id: BufferId,
        timestamp: SystemTime,
    ) -> Self {
        let text = lines.join("\n");
        let rope = if text.is_empty() {
            Rope::new()
        } else {
            Rope::from_str(&text)
        };
        Self {
            content: SnapshotContent::Rope(rope),
            cursor,
            buffer_id,
            timestamp,
        }
    }

    /// Restore a Rope-based buffer to this snapshot's state.
    ///
    /// Returns the cursor position that should be set on Window.
    /// The buffer ID is not changed.
    ///
    /// # Panics
    ///
    /// Panics if this snapshot was captured from a `VirtualBuffer`.
    pub fn restore(&self, buffer: &mut Buffer) -> Position {
        match &self.content {
            SnapshotContent::Rope(rope) => {
                buffer.set_rope(rope.clone());
            }
            SnapshotContent::Virtual(_) => {
                panic!("cannot restore VirtualSnapshot to Rope buffer");
            }
        }
        self.cursor
    }

    /// Restore a `VirtualBuffer` to this snapshot's state.
    ///
    /// Returns the cursor position that should be set on Window.
    ///
    /// # Panics
    ///
    /// Panics if this snapshot was captured from a Rope buffer.
    pub fn restore_virtual(&self, vbuf: &mut VirtualBuffer) -> Position {
        match &self.content {
            SnapshotContent::Virtual(snap) => {
                vbuf.restore_snapshot(snap.clone());
            }
            SnapshotContent::Rope(_) => {
                panic!("cannot restore Rope snapshot to VirtualBuffer");
            }
        }
        self.cursor
    }

    /// Whether this snapshot was captured from a `VirtualBuffer`.
    #[must_use]
    pub const fn is_virtual(&self) -> bool {
        matches!(self.content, SnapshotContent::Virtual(_))
    }

    /// Get the cursor position from this snapshot.
    ///
    /// Use this when reopening a file that already has its content
    /// loaded - just get the cursor position to set on Window.
    #[must_use]
    pub const fn cursor(&self) -> Position {
        self.cursor
    }

    // === Accessors for driver-layer serialization ===

    /// Collect the captured lines as a `Vec<String>`.
    ///
    /// This allocates — prefer `line(idx)` for individual access.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        match &self.content {
            SnapshotContent::Rope(rope) => (0..rope.line_count())
                .filter_map(|i| rope.line(i).map(String::from))
                .collect(),
            SnapshotContent::Virtual(_) => {
                // VirtualSnapshot doesn't provide line iteration directly.
                // Callers should use buffer_line() via the runtime for virtual buffers.
                Vec::new()
            }
        }
    }

    // NOTE: cursor() method defined above (single accessor, no duplicate)

    /// Get the buffer ID.
    #[must_use]
    pub const fn buffer_id(&self) -> BufferId {
        self.buffer_id
    }

    /// Get the timestamp when snapshot was taken.
    #[must_use]
    pub const fn timestamp(&self) -> SystemTime {
        self.timestamp
    }

    // === Utility Methods ===

    /// Check if this snapshot matches a buffer's ID.
    #[must_use]
    pub fn matches_buffer(&self, buffer: &Buffer) -> bool {
        self.buffer_id == buffer.id()
    }

    /// Check if this snapshot matches a virtual buffer's ID.
    #[must_use]
    pub fn matches_virtual_buffer(&self, vbuf: &VirtualBuffer) -> bool {
        self.buffer_id == vbuf.id()
    }

    /// Get the total number of characters in the snapshot.
    #[must_use]
    pub fn char_count(&self) -> usize {
        match &self.content {
            SnapshotContent::Rope(rope) => rope.char_len(),
            // VirtualSnapshot doesn't store char count directly
            SnapshotContent::Virtual(_) => 0,
        }
    }

    /// Get the number of lines in the snapshot.
    #[must_use]
    pub fn line_count(&self) -> usize {
        match &self.content {
            SnapshotContent::Rope(rope) => rope.line_count(),
            // VirtualSnapshot doesn't expose line count
            SnapshotContent::Virtual(_) => 0,
        }
    }

    /// Check if the snapshot is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match &self.content {
            SnapshotContent::Rope(rope) => rope.is_empty(),
            SnapshotContent::Virtual(_) => false,
        }
    }
}
