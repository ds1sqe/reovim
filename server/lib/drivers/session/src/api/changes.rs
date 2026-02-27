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
mod tests {
    use super::*;

    #[test]
    fn test_empty_changes() {
        let changes = StateChanges::new();
        assert!(!changes.has_changes());
    }

    #[test]
    fn test_mode_change() {
        let mut changes = StateChanges::new();
        changes.record_mode_change();
        assert!(changes.has_changes());
        assert!(changes.mode_changed);
    }

    #[test]
    fn test_cursor_move() {
        let mut changes = StateChanges::new();
        let buffer = BufferId::new();
        changes.record_cursor_move(buffer);
        assert!(changes.has_changes());
        assert!(changes.cursor_moved);
        assert_eq!(changes.affected_buffers.len(), 1);
        assert!(changes.affected_buffers.contains(&buffer));
    }

    #[test]
    fn test_buffer_modified() {
        let mut changes = StateChanges::new();
        let buffer = BufferId::new();
        changes.record_buffer_modified(buffer);
        assert!(changes.has_changes());
        assert!(changes.buffer_modified);
        assert_eq!(changes.modified_buffers.len(), 1);
        assert_eq!(changes.affected_buffers.len(), 1);
    }

    #[test]
    fn test_buffer_lifecycle() {
        let mut changes = StateChanges::new();
        let buffer = BufferId::new();

        changes.record_buffer_created(buffer);
        assert!(changes.has_changes());
        assert_eq!(changes.buffers_created.len(), 1);

        changes.record_buffer_deleted(buffer);
        assert_eq!(changes.buffers_deleted.len(), 1);

        changes.record_buffer_renamed(buffer, "new_name.txt".to_string());
        assert_eq!(changes.buffers_renamed.len(), 1);
    }

    #[test]
    fn test_window_changes() {
        let mut changes = StateChanges::new();
        let window = WindowId::new();

        changes.record_window_created(window);
        assert!(changes.has_changes());
        assert!(changes.window_changed);
        assert_eq!(changes.windows_created.len(), 1);

        changes.record_window_closed(window);
        assert_eq!(changes.windows_closed.len(), 1);
    }

    #[test]
    fn test_focus_change() {
        let mut changes = StateChanges::new();
        changes.record_focus_change();
        assert!(changes.has_changes());
        assert!(changes.focus_changed);
    }

    #[test]
    fn test_selection_change() {
        let mut changes = StateChanges::new();
        let buffer = BufferId::new();
        changes.record_selection_change(buffer);
        assert!(changes.has_changes());
        assert!(changes.selection_changed);
        assert!(changes.affected_buffers.contains(&buffer));
    }

    #[test]
    fn test_merge() {
        let mut a = StateChanges::new();
        a.record_mode_change();

        let mut b = StateChanges::new();
        let buffer = BufferId::new();
        b.record_cursor_move(buffer);

        a.merge(b);
        assert!(a.mode_changed);
        assert!(a.cursor_moved);
        assert_eq!(a.affected_buffers.len(), 1);
    }

    #[test]
    fn test_no_duplicate_affected_buffers() {
        let mut changes = StateChanges::new();
        let buffer = BufferId::new();

        // Multiple operations on same buffer
        changes.record_cursor_move(buffer);
        changes.record_cursor_move(buffer);
        changes.record_buffer_modified(buffer);

        // Should only appear once
        assert_eq!(changes.affected_buffers.len(), 1);
    }

    #[test]
    fn test_default() {
        let changes = StateChanges::default();
        assert!(!changes.has_changes());
        assert!(!changes.mode_changed);
        assert!(!changes.cursor_moved);
        assert!(changes.modified_buffers.is_empty());
    }

    // === Option change tests (#445) ===

    #[test]
    fn test_option_change_global() {
        let change = OptionChange::global("number", OptionValue::bool(true));
        assert_eq!(change.name, "number");
        assert_eq!(change.value, OptionValue::bool(true));
        assert!(change.window_id.is_none());
    }

    #[test]
    fn test_option_change_window() {
        let window = WindowId::new();
        let change = OptionChange::window("relativenumber", OptionValue::bool(true), window);
        assert_eq!(change.name, "relativenumber");
        assert_eq!(change.window_id, Some(window));
    }

    #[test]
    fn test_record_option_change() {
        let mut changes = StateChanges::new();
        assert!(!changes.has_changes());
        assert!(!changes.option_changed);

        changes.record_global_option_change("number", OptionValue::bool(true));

        assert!(changes.has_changes());
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].name, "number");
    }

    #[test]
    fn test_record_window_option_change() {
        let mut changes = StateChanges::new();
        let window = WindowId::new();

        changes.record_window_option_change("number", OptionValue::bool(true), window);

        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].window_id, Some(window));
    }

    #[test]
    fn test_merge_option_changes() {
        let mut a = StateChanges::new();
        a.record_global_option_change("number", OptionValue::bool(true));

        let mut b = StateChanges::new();
        b.record_global_option_change("relativenumber", OptionValue::bool(false));

        a.merge(b);
        assert!(a.option_changed);
        assert_eq!(a.options_changed.len(), 2);
    }

    // === Presence change tests (Phase 14) ===

    #[test]
    fn test_presence_change() {
        let mut changes = StateChanges::new();
        assert!(!changes.has_changes());
        assert!(!changes.presence_changed);

        changes.record_presence_change(42);

        assert!(changes.has_changes());
        assert!(changes.presence_changed);
        assert_eq!(changes.presence_updates.len(), 1);
        assert!(changes.presence_updates.contains(&42));
    }

    #[test]
    fn test_presence_change_no_duplicates() {
        let mut changes = StateChanges::new();

        changes.record_presence_change(42);
        changes.record_presence_change(42);
        changes.record_presence_change(42);

        assert_eq!(changes.presence_updates.len(), 1);
    }

    #[test]
    fn test_merge_presence_changes() {
        let mut a = StateChanges::new();
        a.record_presence_change(1);

        let mut b = StateChanges::new();
        b.record_presence_change(2);
        b.record_presence_change(1); // Duplicate

        a.merge(b);
        assert!(a.presence_changed);
        assert_eq!(a.presence_updates.len(), 2);
        assert!(a.presence_updates.contains(&1));
        assert!(a.presence_updates.contains(&2));
    }

    // === Scroll change tests ===

    #[test]
    fn test_scroll_change() {
        let mut changes = StateChanges::new();
        let window = WindowId::new();

        assert!(!changes.scroll_changed);
        assert!(changes.scrolled_windows.is_empty());

        changes.record_scroll_change(window);
        assert!(changes.has_changes());
        assert!(changes.scroll_changed);
        assert_eq!(changes.scrolled_windows.len(), 1);
        assert!(changes.scrolled_windows.contains(&window));
    }

    #[test]
    fn test_scroll_change_no_duplicates() {
        let mut changes = StateChanges::new();
        let window = WindowId::new();

        changes.record_scroll_change(window);
        changes.record_scroll_change(window);
        changes.record_scroll_change(window);

        assert_eq!(changes.scrolled_windows.len(), 1);
    }

    #[test]
    fn test_scroll_change_multiple_windows() {
        let mut changes = StateChanges::new();
        let w1 = WindowId::new();
        let w2 = WindowId::new();

        changes.record_scroll_change(w1);
        changes.record_scroll_change(w2);

        assert_eq!(changes.scrolled_windows.len(), 2);
        assert!(changes.scrolled_windows.contains(&w1));
        assert!(changes.scrolled_windows.contains(&w2));
    }

    #[test]
    fn test_merge_scroll_changes() {
        let mut a = StateChanges::new();
        let w1 = WindowId::new();
        a.record_scroll_change(w1);

        let mut b = StateChanges::new();
        let w2 = WindowId::new();
        b.record_scroll_change(w2);

        a.merge(b);
        assert!(a.scroll_changed);
        assert_eq!(a.scrolled_windows.len(), 2);
    }

    // === Comprehensive merge tests ===

    #[test]
    fn test_merge_all_fields() {
        let mut a = StateChanges::new();
        let buf1 = BufferId::new();
        let win1 = WindowId::new();
        a.record_mode_change();
        a.record_cursor_move(buf1);
        a.record_buffer_modified(buf1);
        a.record_buffer_created(buf1);
        a.record_window_created(win1);
        a.record_focus_change();
        a.record_selection_change(buf1);
        a.record_scroll_change(win1);
        a.record_presence_change(1);

        let mut b = StateChanges::new();
        let buf2 = BufferId::new();
        let win2 = WindowId::new();
        b.record_buffer_deleted(buf2);
        b.record_buffer_renamed(buf2, "renamed.txt".to_string());
        b.record_window_closed(win2);
        b.record_global_option_change("test", OptionValue::bool(true));
        b.record_scroll_change(win2);
        b.record_presence_change(2);
        a.record_extension_change("cmdline".into());
        b.record_extension_change("which-key".into());

        a.merge(b);

        // All flags should be set
        assert!(a.mode_changed);
        assert!(a.cursor_moved);
        assert!(a.selection_changed);
        assert!(a.buffer_modified);
        assert!(a.window_changed);
        assert!(a.focus_changed);
        assert!(a.option_changed);
        assert!(a.scroll_changed);
        assert!(a.presence_changed);
        assert!(a.extension_changed);

        // Collections should have merged
        assert!(!a.modified_buffers.is_empty());
        assert!(!a.buffers_created.is_empty());
        assert!(!a.buffers_deleted.is_empty());
        assert!(!a.buffers_renamed.is_empty());
        assert!(!a.windows_created.is_empty());
        assert!(!a.windows_closed.is_empty());
        assert!(!a.options_changed.is_empty());
        assert_eq!(a.scrolled_windows.len(), 2);
        assert_eq!(a.presence_updates.len(), 2);
        assert_eq!(a.extensions_updated.len(), 2);
    }

    #[test]
    fn test_has_changes_each_field_individually() {
        // Test that each individual field can trigger has_changes()

        // selection_changed
        let mut c = StateChanges::new();
        c.selection_changed = true;
        assert!(c.has_changes());

        // buffers_created
        let mut c = StateChanges::new();
        c.buffers_created.push(BufferId::new());
        assert!(c.has_changes());

        // buffers_deleted
        let mut c = StateChanges::new();
        c.buffers_deleted.push(BufferId::new());
        assert!(c.has_changes());

        // buffers_renamed
        let mut c = StateChanges::new();
        c.buffers_renamed
            .push((BufferId::new(), "test.txt".to_string()));
        assert!(c.has_changes());

        // windows_created
        let mut c = StateChanges::new();
        c.windows_created.push(WindowId::new());
        assert!(c.has_changes());

        // windows_closed
        let mut c = StateChanges::new();
        c.windows_closed.push(WindowId::new());
        assert!(c.has_changes());

        // scroll_changed
        let mut c = StateChanges::new();
        c.scroll_changed = true;
        assert!(c.has_changes());

        // presence_changed
        let mut c = StateChanges::new();
        c.presence_changed = true;
        assert!(c.has_changes());
    }

    #[test]
    fn test_no_duplicate_modified_buffers() {
        let mut changes = StateChanges::new();
        let buffer = BufferId::new();

        changes.record_buffer_modified(buffer);
        changes.record_buffer_modified(buffer);
        changes.record_buffer_modified(buffer);

        assert_eq!(changes.modified_buffers.len(), 1);
        assert_eq!(changes.affected_buffers.len(), 1);
    }

    #[test]
    fn test_no_duplicate_selection_affected_buffers() {
        let mut changes = StateChanges::new();
        let buffer = BufferId::new();

        changes.record_selection_change(buffer);
        changes.record_selection_change(buffer);

        // affected_buffers should only contain the buffer once
        assert_eq!(changes.affected_buffers.len(), 1);
    }

    #[test]
    fn test_record_option_change_direct() {
        let mut changes = StateChanges::new();
        let change = OptionChange::global("test_opt", OptionValue::bool(false));
        changes.record_option_change(change);
        assert!(changes.option_changed);
        assert_eq!(changes.options_changed.len(), 1);
        assert_eq!(changes.options_changed[0].name, "test_opt");
    }

    #[test]
    fn test_multiple_different_buffers_affected() {
        let mut changes = StateChanges::new();
        let buf1 = BufferId::new();
        let buf2 = BufferId::new();
        let buf3 = BufferId::new();

        changes.record_cursor_move(buf1);
        changes.record_buffer_modified(buf2);
        changes.record_selection_change(buf3);

        assert_eq!(changes.affected_buffers.len(), 3);
    }

    #[test]
    fn test_merge_empty_into_populated() {
        let mut a = StateChanges::new();
        a.record_mode_change();
        a.record_cursor_move(BufferId::new());

        let b = StateChanges::new();
        a.merge(b);

        // a should be unchanged
        assert!(a.mode_changed);
        assert!(a.cursor_moved);
    }

    #[test]
    fn test_merge_populated_into_empty() {
        let mut a = StateChanges::new();
        let mut b = StateChanges::new();
        b.record_mode_change();
        b.record_focus_change();

        a.merge(b);
        assert!(a.mode_changed);
        assert!(a.focus_changed);
    }

    // === Extension change tests (#514) ===

    #[test]
    fn test_extension_change() {
        let mut changes = StateChanges::new();
        assert!(!changes.has_changes());
        assert!(!changes.extension_changed);

        changes.record_extension_change("cmdline".into());

        assert!(changes.has_changes());
        assert!(changes.extension_changed);
        assert_eq!(changes.extensions_updated.len(), 1);
        assert!(changes.extensions_updated.contains(&"cmdline".to_string()));
    }

    #[test]
    fn test_extension_change_no_duplicates() {
        let mut changes = StateChanges::new();

        changes.record_extension_change("cmdline".into());
        changes.record_extension_change("cmdline".into());
        changes.record_extension_change("cmdline".into());

        assert_eq!(changes.extensions_updated.len(), 1);
    }

    #[test]
    fn test_extension_change_multiple_kinds() {
        let mut changes = StateChanges::new();

        changes.record_extension_change("cmdline".into());
        changes.record_extension_change("which-key".into());

        assert_eq!(changes.extensions_updated.len(), 2);
        assert!(changes.extensions_updated.contains(&"cmdline".to_string()));
        assert!(
            changes
                .extensions_updated
                .contains(&"which-key".to_string())
        );
    }

    #[test]
    fn test_merge_extension_changes() {
        let mut a = StateChanges::new();
        a.record_extension_change("cmdline".into());

        let mut b = StateChanges::new();
        b.record_extension_change("which-key".into());
        b.record_extension_change("cmdline".into()); // Duplicate

        a.merge(b);
        assert!(a.extension_changed);
        assert_eq!(a.extensions_updated.len(), 2);
        assert!(a.extensions_updated.contains(&"cmdline".to_string()));
        assert!(a.extensions_updated.contains(&"which-key".to_string()));
    }

    #[test]
    fn test_has_changes_extension_changed() {
        let mut c = StateChanges::new();
        c.extension_changed = true;
        assert!(c.has_changes());
    }
}
