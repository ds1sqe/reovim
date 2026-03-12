//! Change tracking for session operations.
//!
//! This module provides [`StateChanges`], which tracks what changed during
//! session operations. The runner uses this to know what notifications to
//! send to clients.
//!
//! # Design
//!
//! Following the mechanism vs policy principle:
//! - **Mechanism**: `StateChanges` tracks WHAT changed
//! - **Policy**: Runner decides HOW to notify clients
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::StateChanges;
//!
//! let mut changes = StateChanges::new();
//! changes.record_mode_change();
//! changes.record_cursor_move(buffer_id);
//!
//! if changes.has_changes() {
//!     // Runner broadcasts notifications
//! }
//! ```

use reovim_kernel::api::v1::{BufferId, OptionValue, WindowId};

/// Represents a single option change.
#[derive(Debug, Clone)]
pub struct OptionChange {
    /// Name of the option that changed.
    pub name: String,
    /// New value.
    pub value: OptionValue,
    /// Window ID if window-scoped, None if global.
    pub window_id: Option<WindowId>,
}

impl OptionChange {
    /// Create a global option change.
    #[must_use]
    pub fn global(name: impl Into<String>, value: OptionValue) -> Self {
        Self {
            name: name.into(),
            value,
            window_id: None,
        }
    }

    /// Create a window-scoped option change.
    #[must_use]
    pub fn window(name: impl Into<String>, value: OptionValue, window_id: WindowId) -> Self {
        Self {
            name: name.into(),
            value,
            window_id: Some(window_id),
        }
    }
}

/// Tracks what changed during an operation.
///
/// Runner uses this to know what notifications to send to clients.
/// All changes are accumulated internally and taken at the end of
/// an operation via [`ChangeTracker::take_changes`].
///
/// The multiple boolean flags are intentional - each tracks a distinct
/// notification type that clients may need to receive.
#[derive(Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct StateChanges {
    // === Mode Changes ===
    /// Whether the mode changed.
    pub mode_changed: bool,

    // === Cursor/Selection Changes ===
    /// Whether the cursor moved.
    pub cursor_moved: bool,
    /// Whether the selection changed.
    pub selection_changed: bool,

    // === Buffer Content Changes ===
    /// Whether any buffer content was modified.
    pub buffer_modified: bool,
    /// Buffers whose content was modified.
    pub modified_buffers: Vec<BufferId>,
    /// All buffers affected (for cursor, selection, etc.).
    pub affected_buffers: Vec<BufferId>,

    // === Buffer Lifecycle Changes ===
    /// Buffers that were created.
    pub buffers_created: Vec<BufferId>,
    /// Buffers that were deleted.
    pub buffers_deleted: Vec<BufferId>,
    /// Buffers that were renamed: (id, `new_name`).
    pub buffers_renamed: Vec<(BufferId, String)>,

    // === Window Changes ===
    /// Whether window layout changed.
    pub window_changed: bool,
    /// Windows that were created.
    pub windows_created: Vec<WindowId>,
    /// Windows that were closed.
    pub windows_closed: Vec<WindowId>,
    /// Whether window focus changed.
    pub focus_changed: bool,

    // === Option Changes (#445) ===
    /// Whether any option changed.
    pub option_changed: bool,
    /// Options that changed.
    /// Each entry contains name, value, and optional `window_id`.
    pub options_changed: Vec<OptionChange>,

    // === Scroll Changes ===
    /// Whether scroll position changed in any window.
    pub scroll_changed: bool,
    /// Windows whose scroll position changed.
    pub scrolled_windows: Vec<WindowId>,

    // === Presence Changes (Phase 14) ===
    // Note: Presence RPCs emit notifications directly (like CaptureRequest).
    // These fields are for future cursor sync scenarios where cursor movement
    // might trigger presence updates.
    /// Whether presence state changed (for cursor sync scenarios).
    pub presence_changed: bool,
    /// Client IDs whose presence changed.
    pub presence_updates: Vec<usize>,

    // === Extension Changes (#514) ===
    /// Whether any extension state changed (activation/deactivation).
    pub extension_changed: bool,
    /// Extension kinds that changed (e.g., `["cmdline"]`).
    pub extensions_updated: Vec<String>,

    // === Lifecycle Signals (#547) ===
    /// Whether a quit was requested during this operation.
    ///
    /// Set when a command pushes `RuntimeSignal::Quit`. Not included in
    /// `has_changes()` — quit is a lifecycle signal, not a state change
    /// that triggers notifications.
    pub should_quit: bool,
}

impl StateChanges {
    /// Create empty changes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any changes occurred.
    #[must_use]
    pub const fn has_changes(&self) -> bool {
        self.mode_changed
            || self.cursor_moved
            || self.selection_changed
            || self.buffer_modified
            || !self.buffers_created.is_empty()
            || !self.buffers_deleted.is_empty()
            || !self.buffers_renamed.is_empty()
            || self.window_changed
            || !self.windows_created.is_empty()
            || !self.windows_closed.is_empty()
            || self.focus_changed
            || self.option_changed
            || self.scroll_changed
            || self.presence_changed
            || self.extension_changed
    }

    /// Merge another `StateChanges` into this one.
    pub fn merge(&mut self, other: Self) {
        self.mode_changed |= other.mode_changed;
        self.cursor_moved |= other.cursor_moved;
        self.selection_changed |= other.selection_changed;
        self.buffer_modified |= other.buffer_modified;
        self.modified_buffers.extend(other.modified_buffers);
        self.affected_buffers.extend(other.affected_buffers);
        self.buffers_created.extend(other.buffers_created);
        self.buffers_deleted.extend(other.buffers_deleted);
        self.buffers_renamed.extend(other.buffers_renamed);
        self.window_changed |= other.window_changed;
        self.windows_created.extend(other.windows_created);
        self.windows_closed.extend(other.windows_closed);
        self.focus_changed |= other.focus_changed;
        self.option_changed |= other.option_changed;
        self.options_changed.extend(other.options_changed);
        self.scroll_changed |= other.scroll_changed;
        self.scrolled_windows.extend(other.scrolled_windows);
        // Phase 14: Presence changes
        self.presence_changed |= other.presence_changed;
        for client_id in other.presence_updates {
            if !self.presence_updates.contains(&client_id) {
                self.presence_updates.push(client_id);
            }
        }
        // #514: Extension changes
        self.extension_changed |= other.extension_changed;
        for kind in other.extensions_updated {
            if !self.extensions_updated.contains(&kind) {
                self.extensions_updated.push(kind);
            }
        }
        // #547: Lifecycle signals
        self.should_quit |= other.should_quit;
    }

    // === Recording helpers ===

    /// Record that the mode changed.
    pub const fn record_mode_change(&mut self) {
        self.mode_changed = true;
    }

    /// Record that the cursor moved in a buffer.
    pub fn record_cursor_move(&mut self, buffer: BufferId) {
        self.cursor_moved = true;
        if !self.affected_buffers.contains(&buffer) {
            self.affected_buffers.push(buffer);
        }
    }

    /// Record that buffer content was modified.
    pub fn record_buffer_modified(&mut self, buffer: BufferId) {
        self.buffer_modified = true;
        if !self.modified_buffers.contains(&buffer) {
            self.modified_buffers.push(buffer);
        }
        if !self.affected_buffers.contains(&buffer) {
            self.affected_buffers.push(buffer);
        }
    }

    /// Record that a buffer was created.
    pub fn record_buffer_created(&mut self, buffer: BufferId) {
        self.buffers_created.push(buffer);
    }

    /// Record that a buffer was deleted.
    pub fn record_buffer_deleted(&mut self, buffer: BufferId) {
        self.buffers_deleted.push(buffer);
    }

    /// Record that a buffer was renamed.
    pub fn record_buffer_renamed(&mut self, buffer: BufferId, new_name: String) {
        self.buffers_renamed.push((buffer, new_name));
    }

    /// Record that a window was created.
    pub fn record_window_created(&mut self, window: WindowId) {
        self.window_changed = true;
        self.windows_created.push(window);
    }

    /// Record that a window was closed.
    pub fn record_window_closed(&mut self, window: WindowId) {
        self.window_changed = true;
        self.windows_closed.push(window);
    }

    /// Record that window focus changed.
    pub const fn record_focus_change(&mut self) {
        self.focus_changed = true;
    }

    /// Record that selection changed in a buffer.
    pub fn record_selection_change(&mut self, buffer: BufferId) {
        self.selection_changed = true;
        if !self.affected_buffers.contains(&buffer) {
            self.affected_buffers.push(buffer);
        }
    }

    /// Record that an option changed.
    pub fn record_option_change(&mut self, change: OptionChange) {
        self.option_changed = true;
        self.options_changed.push(change);
    }

    /// Record a global option change.
    pub fn record_global_option_change(&mut self, name: impl Into<String>, value: OptionValue) {
        self.record_option_change(OptionChange::global(name, value));
    }

    /// Record a window-scoped option change.
    pub fn record_window_option_change(
        &mut self,
        name: impl Into<String>,
        value: OptionValue,
        window_id: WindowId,
    ) {
        self.record_option_change(OptionChange::window(name, value, window_id));
    }

    /// Record that scroll position changed in a window.
    pub fn record_scroll_change(&mut self, window: WindowId) {
        self.scroll_changed = true;
        if !self.scrolled_windows.contains(&window) {
            self.scrolled_windows.push(window);
        }
    }

    /// Record that presence state changed for a client (Phase 14).
    ///
    /// Used for future cursor sync scenarios where cursor movement
    /// might trigger presence updates.
    pub fn record_presence_change(&mut self, client_id: usize) {
        self.presence_changed = true;
        if !self.presence_updates.contains(&client_id) {
            self.presence_updates.push(client_id);
        }
    }

    /// Record that an extension's state changed (#514).
    ///
    /// Called when a bridge's `is_active()` changes (activation/deactivation).
    pub fn record_extension_change(&mut self, kind: String) {
        self.extension_changed = true;
        if !self.extensions_updated.contains(&kind) {
            self.extensions_updated.push(kind);
        }
    }

    /// Record that a quit was requested (#547).
    ///
    /// Called when a command pushes `RuntimeSignal::Quit`. The runner
    /// uses this to signal the client to disconnect.
    pub const fn record_quit_requested(&mut self) {
        self.should_quit = true;
    }
}

/// Trait for collecting accumulated changes.
///
/// Session runtime implements this to allow the runner to take
/// all accumulated changes at the end of an operation.
///
/// # Recording Changes
///
/// Commands can record changes via this trait. The runner collects
/// all changes at the end of an operation to send notifications.
pub trait ChangeTracker: Send {
    /// Take all accumulated changes, resetting internal state.
    fn take_changes(&mut self) -> StateChanges;

    /// Record that the cursor moved in a buffer.
    ///
    /// Commands should call this after moving the cursor via
    /// `BufferApi::set_cursor_position()`.
    fn record_cursor_move(&mut self, buffer: BufferId);

    /// Record that the selection changed in a buffer (#474).
    ///
    /// Commands should call this after creating, modifying, or clearing
    /// a visual selection. The notification pipeline uses this to
    /// broadcast selection state to other clients.
    fn record_selection_change(&mut self, buffer: BufferId);
}
#[cfg(test)]
#[path = "tests/changes.rs"]
mod tests;
