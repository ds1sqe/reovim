//! Bufferline state types.
//!
//! `BufferlineState` holds the pin list (session-wide shared extension).

use reovim_driver_text_session::SessionExtension;

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
