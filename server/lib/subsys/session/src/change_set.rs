//! Domain-neutral change tracking for session operations.
//!
//! [`ChangeSet`] is returned by domain drivers after each operation (key dispatch,
//! command execution). The server reads the flags and ID lists to determine what
//! to broadcast to connected clients.
//!
//! # Design
//!
//! `ChangeSet` carries **signals**, not data. It says "cursor moved" or "buffer
//! modified" — the server then queries the domain for updated values. This keeps
//! all domain-specific data (text edits, vertex moves) inside the domain driver.

use reovim_kernel::api::v1::{BufferId, WindowId};

/// Domain-neutral change set returned by `DomainDriver` after each operation.
///
/// The server inspects this to decide what notifications to broadcast:
/// - `modified_buffers` → re-render buffer content for connected clients
/// - `cursor_moved` → re-query domain for updated cursor positions
/// - `mode_changed` → re-query domain for mode display name
/// - `layout_changed` → re-render window layout
///
/// # What is NOT here
///
/// Domain-specific change details stay inside the domain driver:
/// - `TextBufferModified { buffer_id, edits }` → text-domain internal
/// - `ByteEdit { offset, old, new }` → codec-domain internal
/// - Presence updates → mechanism handles from cursor state
/// - Concrete cursor/position values → server re-queries domain
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone)]
pub struct ChangeSet {
    /// Buffers whose content was modified.
    pub modified_buffers: Vec<BufferId>,
    /// Buffers that were created.
    pub created_buffers: Vec<BufferId>,
    /// Buffers that were deleted (removed from domain).
    pub deleted_buffers: Vec<BufferId>,
    /// Buffers that were closed (removed from view but may still exist).
    pub closed_buffers: Vec<BufferId>,

    /// Cursor position changed — server should re-query domain for cursors.
    pub cursor_moved: bool,

    /// Active mode changed — server should re-query domain for mode display.
    pub mode_changed: bool,

    /// Window layout changed.
    pub layout_changed: bool,
    /// Windows that were created.
    pub created_windows: Vec<WindowId>,
    /// Windows that were closed.
    pub closed_windows: Vec<WindowId>,
    /// Focus moved to a different window.
    pub focus_changed: bool,

    /// Scroll position changed.
    pub scroll_changed: bool,
    /// Windows whose scroll position changed.
    pub scrolled_windows: Vec<WindowId>,

    /// Editor options changed.
    pub options_changed: bool,

    /// Client should quit.
    pub should_quit: bool,
    /// Client should detach (disconnect but server keeps running).
    pub should_detach: bool,
}

impl ChangeSet {
    /// Create an empty change set with no changes recorded.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether any changes were recorded.
    #[must_use]
    pub const fn has_changes(&self) -> bool {
        self.cursor_moved
            || self.mode_changed
            || self.layout_changed
            || self.focus_changed
            || self.scroll_changed
            || self.options_changed
            || self.should_quit
            || self.should_detach
            || !self.modified_buffers.is_empty()
            || !self.created_buffers.is_empty()
            || !self.deleted_buffers.is_empty()
            || !self.closed_buffers.is_empty()
            || !self.created_windows.is_empty()
            || !self.closed_windows.is_empty()
            || !self.scrolled_windows.is_empty()
    }

    /// Merge another change set into this one.
    ///
    /// Boolean flags are OR'd. ID lists are extended (no deduplication —
    /// callers that need unique IDs should deduplicate after merging).
    pub fn merge(&mut self, other: Self) {
        self.modified_buffers.extend(other.modified_buffers);
        self.created_buffers.extend(other.created_buffers);
        self.deleted_buffers.extend(other.deleted_buffers);
        self.closed_buffers.extend(other.closed_buffers);

        self.cursor_moved |= other.cursor_moved;
        self.mode_changed |= other.mode_changed;

        self.layout_changed |= other.layout_changed;
        self.created_windows.extend(other.created_windows);
        self.closed_windows.extend(other.closed_windows);
        self.focus_changed |= other.focus_changed;

        self.scroll_changed |= other.scroll_changed;
        self.scrolled_windows.extend(other.scrolled_windows);

        self.options_changed |= other.options_changed;
        self.should_quit |= other.should_quit;
        self.should_detach |= other.should_detach;
    }

    /// Record that a buffer was modified.
    pub fn record_buffer_modified(&mut self, buffer_id: BufferId) {
        self.modified_buffers.push(buffer_id);
    }

    /// Record that a buffer was created.
    pub fn record_buffer_created(&mut self, buffer_id: BufferId) {
        self.created_buffers.push(buffer_id);
    }

    /// Record that a buffer was deleted.
    pub fn record_buffer_deleted(&mut self, buffer_id: BufferId) {
        self.deleted_buffers.push(buffer_id);
    }

    /// Record that a buffer was closed.
    pub fn record_buffer_closed(&mut self, buffer_id: BufferId) {
        self.closed_buffers.push(buffer_id);
    }

    /// Record that the cursor moved.
    pub const fn record_cursor_move(&mut self) {
        self.cursor_moved = true;
    }

    /// Record that the mode changed.
    pub const fn record_mode_change(&mut self) {
        self.mode_changed = true;
    }

    /// Record that the layout changed.
    pub const fn record_layout_change(&mut self) {
        self.layout_changed = true;
    }

    /// Record that a window was created.
    pub fn record_window_created(&mut self, window_id: WindowId) {
        self.layout_changed = true;
        self.created_windows.push(window_id);
    }

    /// Record that a window was closed.
    pub fn record_window_closed(&mut self, window_id: WindowId) {
        self.layout_changed = true;
        self.closed_windows.push(window_id);
    }

    /// Record that focus changed.
    pub const fn record_focus_change(&mut self) {
        self.focus_changed = true;
    }

    /// Record that a window scrolled.
    pub fn record_scroll_change(&mut self, window_id: WindowId) {
        self.scroll_changed = true;
        self.scrolled_windows.push(window_id);
    }

    /// Record that options changed.
    pub const fn record_options_change(&mut self) {
        self.options_changed = true;
    }

    /// Record that the client should quit.
    pub const fn record_quit(&mut self) {
        self.should_quit = true;
    }

    /// Record that the client should detach.
    pub const fn record_detach(&mut self) {
        self.should_detach = true;
    }
}
