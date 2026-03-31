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
    crate::mm::{Buffer, BufferId, Position, Rope},
    std::time::SystemTime,
};

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
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Text content at time of capture (rope clone, O(1)).
    text: Rope,
    /// Cursor position at time of capture.
    cursor: Position,
    /// Buffer ID (for validation on restore).
    buffer_id: BufferId,
    /// When snapshot was taken.
    timestamp: SystemTime,
}

impl Snapshot {
    /// Capture the current state of a buffer.
    ///
    /// Cursor position must be passed explicitly - get it from Window.
    /// This is O(1) — the rope is cloned via `Arc` sharing.
    #[must_use]
    pub fn capture(buffer: &Buffer, cursor: Position) -> Self {
        Self {
            text: buffer.clone_rope(),
            cursor,
            buffer_id: buffer.id(),
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
        let content = lines.join("\n");
        let text = if content.is_empty() {
            Rope::new()
        } else {
            Rope::from_str(&content)
        };
        Self {
            text,
            cursor,
            buffer_id,
            timestamp,
        }
    }

    /// Restore a buffer to this snapshot's state.
    ///
    /// Returns the cursor position that should be set on Window.
    /// The buffer ID is not changed.
    pub fn restore(&self, buffer: &mut Buffer) -> Position {
        // O(1) restore via rope clone (Arc sharing)
        buffer.set_rope(self.text.clone());

        // Return cursor position for caller to set on Window
        self.cursor
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
        (0..self.text.line_count())
            .filter_map(|i| self.text.line(i).map(String::from))
            .collect()
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

    /// Get the total number of characters in the snapshot.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.text.char_len()
    }

    /// Get the number of lines in the snapshot.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.text.line_count()
    }

    /// Check if the snapshot is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}
