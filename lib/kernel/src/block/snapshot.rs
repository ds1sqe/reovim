//! Buffer state capture and restore.
//!
//! Snapshots capture the complete state of a buffer at a point in time,
//! enabling restore operations for recovery or checkpointing.
//!
//! # Design Principle
//!
//! Following the kernel purity principle, this module provides pure Rust
//! data structures and accessors. File serialization (JSON, etc.) is
//! handled by the driver layer (`lib/drivers/vfs/`).

use {
    crate::mm::{Buffer, BufferId, Position},
    std::time::SystemTime,
};

/// A captured buffer state.
///
/// Snapshots are used for:
/// - Safety checkpoints before complex operations
/// - Crash recovery
/// - Cursor restoration when reopening files
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
///
/// // Capture state
/// let snapshot = Snapshot::capture(&buffer);
///
/// // Later, restore the state
/// snapshot.restore(&mut buffer);
/// ```
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Lines at time of capture.
    lines: Vec<String>,
    /// Cursor position at time of capture.
    cursor: Position,
    /// Buffer ID (for validation on restore).
    buffer_id: BufferId,
    /// When snapshot was taken.
    timestamp: SystemTime,
}

impl Snapshot {
    /// Capture the current state of a buffer.
    #[must_use]
    pub fn capture(buffer: &Buffer) -> Self {
        Self {
            lines: buffer.lines().to_vec(),
            cursor: buffer.position(),
            buffer_id: buffer.id(),
            timestamp: SystemTime::now(),
        }
    }

    /// Create a snapshot from components.
    ///
    /// This is useful for restoring from serialized data.
    #[must_use]
    pub const fn from_parts(
        lines: Vec<String>,
        cursor: Position,
        buffer_id: BufferId,
        timestamp: SystemTime,
    ) -> Self {
        Self {
            lines,
            cursor,
            buffer_id,
            timestamp,
        }
    }

    /// Restore a buffer to this snapshot's state.
    ///
    /// This replaces the buffer's content and cursor position.
    /// The buffer ID is not changed.
    pub fn restore(&self, buffer: &mut Buffer) {
        // Set content from snapshot lines
        let content = self.lines.join("\n");
        buffer.set_content(&content);

        // Restore cursor position
        buffer.set_position(self.cursor);
    }

    /// Restore only the cursor position.
    ///
    /// This is useful when reopening a file that already has its
    /// content loaded - we only need to restore the cursor.
    pub fn restore_cursor(&self, buffer: &mut Buffer) {
        buffer.set_position(self.cursor);
    }

    // === Accessors for driver-layer serialization ===

    /// Get the captured lines.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Get the captured cursor position.
    #[must_use]
    pub const fn cursor(&self) -> Position {
        self.cursor
    }

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
        self.lines.iter().map(|l| l.chars().count()).sum::<usize>()
            + self.lines.len().saturating_sub(1) // newlines
    }

    /// Get the number of lines in the snapshot.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Check if the snapshot is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}
