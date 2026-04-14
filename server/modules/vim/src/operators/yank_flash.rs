//! Yank flash state and bridge for client-side yank animations (#657).
//!
//! When a yank operation completes, `YankFlashState` is stored in the
//! per-client `ExtensionMap` with the affected range. The `YankFlashBridge`
//! serializes this state to JSON and delivers it to clients via the existing
//! `extension_updated` notification pipeline. Clients use this to render a
//! brief highlight flash on the yanked text.
//!
//! # Architecture
//!
//! - **Mechanism**: `ExtensionStateBridge` + `extension_updated` pipeline
//! - **Policy**: Client modules decide HOW to render the flash (duration, color)
//! - **No proto changes**: Reuses existing `extension_updated` notification type

use {
    reovim_driver_text_session::{
        ExtensionMap, SessionExtension,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::BufferId,
};

/// Kind identifier for the yank flash bridge.
pub const KIND: &str = "yank-flash";

/// Per-client yank flash state.
///
/// Stored in the per-client `ExtensionMap` after each successful yank.
/// The `sequence` field is a monotonic counter that allows clients to
/// distinguish repeated yanks of the same range.
#[derive(Debug)]
pub struct YankFlashState {
    /// Buffer where the yank occurred.
    pub buffer_id: BufferId,
    /// Start line of the yanked range (0-indexed).
    pub start_line: usize,
    /// End line of the yanked range (0-indexed, inclusive).
    pub end_line: usize,
    /// Start column of the yanked range (0-indexed).
    pub start_col: usize,
    /// End column of the yanked range (0-indexed).
    pub end_col: usize,
    /// Whether the yank was linewise.
    pub is_linewise: bool,
    /// Monotonic counter — increments on every yank.
    pub sequence: u64,
}

impl YankFlashState {
    /// Record a new yank flash, incrementing the sequence counter.
    pub const fn record(
        &mut self,
        buffer_id: BufferId,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        is_linewise: bool,
    ) {
        self.buffer_id = buffer_id;
        self.start_line = start_line;
        self.end_line = end_line;
        self.start_col = start_col;
        self.end_col = end_col;
        self.is_linewise = is_linewise;
        self.sequence += 1;
    }
}

impl SessionExtension for YankFlashState {
    fn create() -> Self {
        Self {
            buffer_id: BufferId::from_raw(0),
            start_line: 0,
            end_line: 0,
            start_col: 0,
            end_col: 0,
            is_linewise: false,
            sequence: 0,
        }
    }
}

/// Bridge for yank flash state.
///
/// Reads [`YankFlashState`] from the client's `ExtensionMap` and serializes
/// it to JSON for consumption by TUI and Web yank-flash extensions.
pub struct YankFlashBridge;

impl ExtensionStateBridge for YankFlashBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<YankFlashState>()?;

        // sequence 0 means no yank has occurred yet
        if state.sequence == 0 {
            return None;
        }

        Some(serde_json::json!({
            "bufferId": state.buffer_id.as_usize(),
            "startLine": state.start_line,
            "endLine": state.end_line,
            "startCol": state.start_col,
            "endCol": state.end_col,
            "isLinewise": state.is_linewise,
            "sequence": state.sequence,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<YankFlashState>()
            .is_some_and(|s| s.sequence > 0)
    }
}

#[cfg(test)]
#[path = "tests/yank_flash.rs"]
mod tests;
