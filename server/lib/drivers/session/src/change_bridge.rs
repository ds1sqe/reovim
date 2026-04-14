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
//! | `selection_changed` | Text-domain | Visual mode concept |
//! | `affected_buffers` | Internal bookkeeping | `modified_buffers` suffices |
//!
//! # Deferred fields (sub-plan 05)
//!
//! The following fields are mechanism-level but don't have `ChangeSet`
//! counterparts yet. They get TODO markers here and are mapped in
//! sub-plan 05 when the server switches to `DomainDriver`:
//!
//! - `buffers_renamed` → needs `ChangeSet::renamed_buffers` field
//! - `presence_changed/updates` → mechanism, maps in sub-plan 05
//! - `extension_changed/updated` → mechanism, maps in sub-plan 05

use {crate::api::StateChanges, reovim_subsys_session::ChangeSet};

/// Convert text-domain `StateChanges` to domain-neutral `ChangeSet`.
///
/// Maps common flags and ID lists directly. Text-specific fields
/// (`text_buffer_edits`, `byte_edits`, `selection_changed`) are dropped.
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

    // ID lists
    cs.modified_buffers.clone_from(&changes.modified_buffers);
    cs.created_buffers.clone_from(&changes.buffers_created);
    cs.deleted_buffers.clone_from(&changes.buffers_deleted);
    cs.created_windows.clone_from(&changes.windows_created);
    cs.closed_windows.clone_from(&changes.windows_closed);
    cs.scrolled_windows.clone_from(&changes.scrolled_windows);

    // --- Explicitly NOT mapped ---
    //
    // Text-domain internal:
    //   changes.text_buffer_edits  → stays in text domain (syntax/codec)
    //   changes.byte_edits         → stays in text domain (codec index)
    //   changes.selection_changed  → text-domain visual mode concept
    //   changes.affected_buffers   → internal bookkeeping
    //
    // TODO(sub-plan-05): Map when ChangeSet gains these fields:
    //   changes.buffers_renamed    → ChangeSet::renamed_buffers
    //   changes.presence_changed   → ChangeSet presence field
    //   changes.presence_updates   → ChangeSet presence field
    //   changes.extension_changed  → ChangeSet extension field
    //   changes.extensions_updated → ChangeSet extension field

    cs
}

#[cfg(test)]
#[path = "change_bridge_tests.rs"]
mod tests;
