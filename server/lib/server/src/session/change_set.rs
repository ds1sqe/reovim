//! Internal server-owned change-set type for notification accumulation.
//!
//! [`ChangeSet`] is a server-internal intermediate used while server-side
//! notification/presence/bridge plumbing still aggregates changes before
//! converting them to protocol notifications.

use {
    reovim_kernel::api::v1::{BufferId, WindowId},
    reovim_subsys_session::OptionChange,
};

/// Internal server change set.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone)]
pub struct ChangeSet {
    pub modified_buffers: Vec<BufferId>,
    pub created_buffers: Vec<BufferId>,
    pub deleted_buffers: Vec<BufferId>,
    pub closed_buffers: Vec<BufferId>,

    pub cursor_moved: bool,
    pub mode_changed: bool,

    pub layout_changed: bool,
    pub created_windows: Vec<WindowId>,
    pub closed_windows: Vec<WindowId>,
    pub focus_changed: bool,

    pub scroll_changed: bool,
    pub scrolled_windows: Vec<WindowId>,

    pub options_changed: bool,
    pub should_quit: bool,
    pub should_detach: bool,

    pub affected_buffers: Vec<BufferId>,
    pub selection_changed: bool,
    pub option_changes: Vec<OptionChange>,
    pub presence_changed: bool,
    pub presence_updates: Vec<usize>,
    pub extension_changed: bool,
    pub extensions_updated: Vec<String>,
    pub renamed_buffers: Vec<(BufferId, String)>,
}

impl ChangeSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

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

    #[cfg(test)]
    pub fn record_buffer_modified(&mut self, buffer_id: BufferId) {
        self.modified_buffers.push(buffer_id);
    }

    #[cfg(test)]
    pub fn record_buffer_created(&mut self, buffer_id: BufferId) {
        self.created_buffers.push(buffer_id);
    }

    #[cfg(test)]
    pub fn record_buffer_deleted(&mut self, buffer_id: BufferId) {
        self.deleted_buffers.push(buffer_id);
    }

    #[cfg(test)]
    pub fn record_buffer_closed(&mut self, buffer_id: BufferId) {
        self.closed_buffers.push(buffer_id);
    }

    pub fn record_cursor_move(&mut self, buffer_id: BufferId) {
        self.cursor_moved = true;
        if !self.affected_buffers.contains(&buffer_id) {
            self.affected_buffers.push(buffer_id);
        }
    }

    #[cfg(test)]
    pub const fn record_mode_change(&mut self) {
        self.mode_changed = true;
    }

    #[cfg(test)]
    pub const fn record_layout_change(&mut self) {
        self.layout_changed = true;
    }

    #[cfg(test)]
    pub fn record_window_created(&mut self, window_id: WindowId) {
        self.layout_changed = true;
        self.created_windows.push(window_id);
    }

    #[cfg(test)]
    pub fn record_window_closed(&mut self, window_id: WindowId) {
        self.layout_changed = true;
        self.closed_windows.push(window_id);
    }

    #[cfg(test)]
    pub const fn record_focus_change(&mut self) {
        self.focus_changed = true;
    }

    #[cfg(test)]
    pub fn record_scroll_change(&mut self, window_id: WindowId) {
        self.scroll_changed = true;
        self.scrolled_windows.push(window_id);
    }

    #[cfg(test)]
    pub fn record_option_change(&mut self, change: OptionChange) {
        self.options_changed = true;
        self.option_changes.push(change);
    }

    #[cfg(test)]
    pub const fn record_quit(&mut self) {
        self.should_quit = true;
    }

    #[cfg(test)]
    pub const fn record_detach(&mut self) {
        self.should_detach = true;
    }

    #[cfg(test)]
    pub fn record_selection_change(&mut self, buffer_id: BufferId) {
        self.selection_changed = true;
        if !self.affected_buffers.contains(&buffer_id) {
            self.affected_buffers.push(buffer_id);
        }
    }

    pub fn record_presence_change(&mut self, client_id: usize) {
        self.presence_changed = true;
        self.presence_updates.push(client_id);
    }

    pub fn record_extension_change(&mut self, kind: String) {
        self.extension_changed = true;
        self.extensions_updated.push(kind);
    }

    #[cfg(test)]
    pub fn record_buffer_renamed(&mut self, buffer_id: BufferId, new_name: String) {
        self.renamed_buffers.push((buffer_id, new_name));
    }
}
