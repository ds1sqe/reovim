//! Bufferline state types.
//!
//! `BufferlineSnapshot` holds the computed buffer entries for client rendering.
//! `BufferlineState` holds the pin list (session-wide shared extension).
//! `BufferEntry` is the per-buffer data structure sent to clients.

use reovim_driver_session::SessionExtension;

/// A single buffer entry in the bufferline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferEntry {
    /// Buffer identifier.
    pub id: u64,
    /// Display name (filename or "[No Name]").
    pub name: String,
    /// Full file path, if file-backed.
    pub path: Option<String>,
    /// Whether the buffer has unsaved changes.
    pub modified: bool,
    /// Whether the buffer is pinned.
    pub pinned: bool,
    /// Filetype (e.g., "rust", "python").
    pub filetype: Option<String>,
    /// Number of error-severity diagnostics.
    pub error_count: u32,
    /// Number of warning-severity diagnostics.
    pub warning_count: u32,
}

/// Computed snapshot of buffer entries for client rendering.
///
/// Stored in the shared `ExtensionMap`. Updated by `BufferlineBridge::tick()`.
pub struct BufferlineSnapshot {
    /// Ordered list of buffer entries.
    pub entries: Vec<BufferEntry>,
}

impl SessionExtension for BufferlineSnapshot {
    fn create() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

/// Session-wide bufferline state (pin list).
///
/// Stored in the shared `ExtensionMap`. Modified by pin/unpin commands.
pub struct BufferlineState {
    /// IDs of pinned buffers, in pin order.
    pub pinned: Vec<u64>,
}

impl SessionExtension for BufferlineState {
    fn create() -> Self {
        Self { pinned: Vec::new() }
    }
}

impl BufferlineState {
    /// Pin a buffer. No-op if already pinned.
    pub fn pin(&mut self, id: u64) {
        if !self.pinned.contains(&id) {
            self.pinned.push(id);
        }
    }

    /// Unpin a buffer. No-op if not pinned.
    pub fn unpin(&mut self, id: u64) {
        self.pinned.retain(|&pid| pid != id);
    }

    /// Check whether a buffer is pinned.
    #[must_use]
    pub fn is_pinned(&self, id: u64) -> bool {
        self.pinned.contains(&id)
    }

    /// Remove pins for buffers no longer in the live set.
    pub fn clean_stale(&mut self, live_ids: &[u64]) {
        self.pinned.retain(|pid| live_ids.contains(pid));
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
