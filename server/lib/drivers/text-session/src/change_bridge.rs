//! `StateChanges` → `ChangeSet` conversion bridge.
//!
//! Converts text-domain [`StateChanges`] to domain-neutral [`ChangeSet`].
//!
//! # What is NOT mapped
//!
//! Text-specific fields stay inside the text domain for internal consumers
//! (syntax highlighting, codec index):
//!
//! | Field | Classification | Disposition |
//! |-------|---------------|-------------|
//! | `text_buffer_edits` | Text-domain | Syntax/codec consumers |
//! | `byte_edits` | Codec-domain | Codec index |
//!
//! These are stored in `SessionState::pending_text_edits` / `pending_byte_edits`
//! by `dispatch_key_for_client` and consumed by the server's syntax/codec paths.
//! Phase 5 will move those consumers into the domain driver.

use {crate::api::StateChanges, reovim_subsys_session::ChangeSet};

/// Convert text-domain `StateChanges` to domain-neutral `ChangeSet`.
///
/// Maps common flags and ID lists directly. Text-specific fields
/// (`text_buffer_edits`, `byte_edits`) are stored separately in
/// `SessionState` by `dispatch_key_for_client`.
#[must_use]
pub fn state_changes_to_change_set(changes: &StateChanges) -> ChangeSet {
    let mut cs = ChangeSet::new();

    // Boolean flags
    cs.cursor_moved = changes.cursor_moved;
    cs.mode_changed = changes.mode_changed;
    cs.layout_changed = changes.window_changed;
    cs.focus_changed = changes.focus_changed;
    cs.scroll_changed = changes.scroll_changed;
    cs.options_changed = changes.option_changed;
    cs.should_quit = changes.should_quit;
    cs.selection_changed = changes.selection_changed;
    cs.presence_changed = changes.presence_changed;
    cs.extension_changed = changes.extension_changed;

    // ID lists
    cs.modified_buffers.clone_from(&changes.modified_buffers);
    cs.created_buffers.clone_from(&changes.buffers_created);
    cs.deleted_buffers.clone_from(&changes.buffers_deleted);
    cs.created_windows.clone_from(&changes.windows_created);
    cs.closed_windows.clone_from(&changes.windows_closed);
    cs.scrolled_windows.clone_from(&changes.scrolled_windows);
    cs.affected_buffers.clone_from(&changes.affected_buffers);
    cs.presence_updates.clone_from(&changes.presence_updates);
    cs.extensions_updated
        .clone_from(&changes.extensions_updated);
    cs.renamed_buffers.clone_from(&changes.buffers_renamed);

    // OptionChange list: same type since driver re-exports from subsys-session
    cs.option_changes.clone_from(&changes.options_changed);

    cs
}

/// Convert a domain-neutral `ChangeSet` back to a text-domain `StateChanges`.
///
/// Used exclusively in test code to convert `dispatch_key_for_client` results
/// (which return `ChangeSet`) into `StateChanges` for accumulation in test
/// helper functions that maintain backwards compatibility.
///
/// This is a lossy conversion: text-domain fields like `text_buffer_edits` and
/// `byte_edits` cannot be reconstructed and are left empty.
#[must_use]
pub fn state_changes_from_change_set(cs: ChangeSet) -> StateChanges {
    let mut sc = StateChanges::new();

    sc.cursor_moved = cs.cursor_moved;
    sc.mode_changed = cs.mode_changed;
    sc.window_changed = cs.layout_changed;
    sc.focus_changed = cs.focus_changed;
    sc.scroll_changed = cs.scroll_changed;
    sc.option_changed = cs.options_changed;
    sc.should_quit = cs.should_quit;
    sc.selection_changed = cs.selection_changed;
    sc.presence_changed = cs.presence_changed;
    sc.extension_changed = cs.extension_changed;

    sc.modified_buffers = cs.modified_buffers;
    sc.buffers_created = cs.created_buffers;
    sc.buffers_deleted = cs.deleted_buffers;
    sc.windows_created = cs.created_windows;
    sc.windows_closed = cs.closed_windows;
    sc.scrolled_windows = cs.scrolled_windows;
    sc.affected_buffers = cs.affected_buffers;
    sc.presence_updates = cs.presence_updates;
    sc.extensions_updated = cs.extensions_updated;
    sc.buffers_renamed = cs.renamed_buffers;
    sc.options_changed = cs.option_changes;

    sc
}

#[cfg(test)]
#[path = "change_bridge_tests.rs"]
mod tests;
