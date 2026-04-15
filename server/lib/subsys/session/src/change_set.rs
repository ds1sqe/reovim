//! Internal change-set type for text-domain migration.
//!
//! [`ChangeSet`] is a text-domain internal type used as an intermediate
//! representation during the migration to `DispatchResult`. It is no longer
//! part of the `DomainDriver` public API — drivers return `DispatchResult`
//! and `CommandResult` instead.
//!
//! # Status
//!
//! This module is `#[doc(hidden)]` and accessible only to drivers mid-migration
//! (e.g., `reovim-driver-text-session`). Once migration is complete it will be
//! removed from this crate entirely.
//!
//! # Design
//!
//! `ChangeSet` carries **signals**, not data. It says "cursor moved" or "buffer
//! modified". All signal flags except buffer lifecycle and session directives
//! are dropped when converting to `DispatchResult`.

use reovim_kernel::api::v1::{BufferId, WindowId};

/// Internal text-domain change set (migration intermediate type).
///
/// Used by `reovim-driver-text-session` as an internal bridge type while
/// migrating from `ChangeSet`-based dispatch to `DispatchResult`-based dispatch.
/// The driver converts this to `DispatchResult` at the public boundary.
///
/// Not part of the `DomainDriver` public API.
///
/// # What is NOT here
///
/// Domain-specific change details stay inside the domain driver:
/// - `TextBufferModified { buffer_id, edits }` → text-domain internal
/// - `ByteEdit { offset, old, new }` → codec-domain internal
/// - Concrete cursor/position values → server polls via `collect_projections`
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

    /// Buffers affected by cursor/selection changes (for per-buffer notifications).
    pub affected_buffers: Vec<BufferId>,

    /// Selection changed — server should re-query domain for selection state.
    pub selection_changed: bool,

    /// Detailed option changes (empty list means no change).
    pub option_changes: Vec<crate::OptionChange>,

    /// Presence state changed (client moved to different buffer/window).
    pub presence_changed: bool,
    /// Client IDs whose presence changed.
    pub presence_updates: Vec<usize>,

    /// Extension state changed (bridge activated/deactivated).
    pub extension_changed: bool,
    /// Extension kind names that changed.
    pub extensions_updated: Vec<String>,

    /// Buffers that were renamed: `(buffer_id, new_name)`.
    pub renamed_buffers: Vec<(BufferId, String)>,
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
            || self.selection_changed
            || self.presence_changed
            || self.extension_changed
            || !self.modified_buffers.is_empty()
            || !self.created_buffers.is_empty()
            || !self.deleted_buffers.is_empty()
            || !self.closed_buffers.is_empty()
            || !self.created_windows.is_empty()
            || !self.closed_windows.is_empty()
            || !self.scrolled_windows.is_empty()
            || !self.renamed_buffers.is_empty()
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

        self.affected_buffers.extend(other.affected_buffers);
        self.selection_changed |= other.selection_changed;
        self.option_changes.extend(other.option_changes);
        self.presence_changed |= other.presence_changed;
        self.presence_updates.extend(other.presence_updates);
        self.extension_changed |= other.extension_changed;
        self.extensions_updated.extend(other.extensions_updated);
        self.renamed_buffers.extend(other.renamed_buffers);
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

    /// Record that the cursor moved in a buffer.
    pub fn record_cursor_move(&mut self, buffer_id: BufferId) {
        self.cursor_moved = true;
        if !self.affected_buffers.contains(&buffer_id) {
            self.affected_buffers.push(buffer_id);
        }
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

    /// Record a single option change.
    pub fn record_option_change(&mut self, change: crate::OptionChange) {
        self.options_changed = true;
        self.option_changes.push(change);
    }

    /// Record that the client should quit.
    pub const fn record_quit(&mut self) {
        self.should_quit = true;
    }

    /// Record that the client should detach.
    pub const fn record_detach(&mut self) {
        self.should_detach = true;
    }

    /// Record that the selection changed in a buffer.
    pub fn record_selection_change(&mut self, buffer_id: BufferId) {
        self.selection_changed = true;
        if !self.affected_buffers.contains(&buffer_id) {
            self.affected_buffers.push(buffer_id);
        }
    }

    /// Record a presence change for a client.
    pub fn record_presence_change(&mut self, client_id: usize) {
        self.presence_changed = true;
        self.presence_updates.push(client_id);
    }

    /// Record an extension state change.
    pub fn record_extension_change(&mut self, kind: String) {
        self.extension_changed = true;
        self.extensions_updated.push(kind);
    }

    /// Record a buffer rename.
    pub fn record_buffer_renamed(&mut self, buffer_id: BufferId, new_name: String) {
        self.renamed_buffers.push((buffer_id, new_name));
    }
}
