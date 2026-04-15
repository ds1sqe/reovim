use reovim_kernel::api::v1::{BufferId, WindowId};

use {super::*, crate::api::StateChanges};

#[test]
fn test_empty_changes() {
    let sc = StateChanges::new();
    let cs = state_changes_to_change_set(&sc);
    assert!(!cs.has_changes());
}

#[test]
fn test_cursor_moved() {
    let mut sc = StateChanges::new();
    let buf = BufferId::new();
    sc.record_cursor_move(buf);

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.cursor_moved);
}

#[test]
fn test_mode_changed() {
    let mut sc = StateChanges::new();
    sc.record_mode_change();

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.mode_changed);
}

#[test]
fn test_layout_changed() {
    let mut sc = StateChanges::new();
    sc.window_changed = true;

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.layout_changed);
}

#[test]
fn test_focus_changed() {
    let mut sc = StateChanges::new();
    sc.record_focus_change();

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.focus_changed);
}

#[test]
fn test_scroll_changed() {
    let mut sc = StateChanges::new();
    let win = WindowId::new();
    sc.record_scroll_change(win);

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.scroll_changed);
    assert_eq!(cs.scrolled_windows.len(), 1);
    assert_eq!(cs.scrolled_windows[0], win);
}

#[test]
fn test_options_changed() {
    let mut sc = StateChanges::new();
    sc.option_changed = true;

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.options_changed);
}

#[test]
fn test_should_quit() {
    let mut sc = StateChanges::new();
    sc.record_quit_requested();

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.should_quit);
}

#[test]
fn test_modified_buffers() {
    let mut sc = StateChanges::new();
    let buf1 = BufferId::new();
    let buf2 = BufferId::new();
    sc.record_buffer_modified(buf1);
    sc.record_buffer_modified(buf2);

    let cs = state_changes_to_change_set(&sc);
    assert_eq!(cs.modified_buffers.len(), 2);
    assert!(cs.modified_buffers.contains(&buf1));
    assert!(cs.modified_buffers.contains(&buf2));
}

#[test]
fn test_created_buffers() {
    let mut sc = StateChanges::new();
    let buf = BufferId::new();
    sc.record_buffer_created(buf);

    let cs = state_changes_to_change_set(&sc);
    assert_eq!(cs.created_buffers.len(), 1);
    assert_eq!(cs.created_buffers[0], buf);
}

#[test]
fn test_deleted_buffers() {
    let mut sc = StateChanges::new();
    let buf = BufferId::new();
    sc.record_buffer_deleted(buf);

    let cs = state_changes_to_change_set(&sc);
    assert_eq!(cs.deleted_buffers.len(), 1);
    assert_eq!(cs.deleted_buffers[0], buf);
}

#[test]
fn test_created_windows() {
    let mut sc = StateChanges::new();
    let win = WindowId::new();
    sc.record_window_created(win);

    let cs = state_changes_to_change_set(&sc);
    assert_eq!(cs.created_windows.len(), 1);
    assert_eq!(cs.created_windows[0], win);
}

#[test]
fn test_closed_windows() {
    let mut sc = StateChanges::new();
    let win = WindowId::new();
    sc.record_window_closed(win);

    let cs = state_changes_to_change_set(&sc);
    assert_eq!(cs.closed_windows.len(), 1);
    assert_eq!(cs.closed_windows[0], win);
}

#[test]
fn test_text_domain_fields_dropped() {
    let mut sc = StateChanges::new();

    // text_buffer_edits and byte_edits have no ChangeSet equivalent —
    // they are stored separately via pending_text_edits / pending_byte_edits.
    sc.buffer_modified = true;
    // Don't populate modified_buffers — only the bool flag is set.

    let cs = state_changes_to_change_set(&sc);
    // buffer_modified bool is not mapped; only modified_buffers list matters.
    assert!(cs.modified_buffers.is_empty());
}

#[test]
fn test_selection_mapped() {
    let mut sc = StateChanges::new();
    let buf = BufferId::new();
    sc.record_selection_change(buf);

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.selection_changed);
    assert_eq!(cs.affected_buffers.len(), 1);
    assert_eq!(cs.affected_buffers[0], buf);
}

#[test]
fn test_presence_mapped() {
    let mut sc = StateChanges::new();
    sc.record_presence_change(1);
    sc.record_presence_change(2);

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.presence_changed);
    assert!(cs.presence_updates.contains(&1));
    assert!(cs.presence_updates.contains(&2));
}

#[test]
fn test_extension_mapped() {
    let mut sc = StateChanges::new();
    sc.record_extension_change("cmdline".to_owned());

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.extension_changed);
    assert!(cs.extensions_updated.contains(&"cmdline".to_string()));
}

#[test]
fn test_renamed_buffers_mapped() {
    let mut sc = StateChanges::new();
    let buf = BufferId::new();
    sc.record_buffer_renamed(buf, "new_name".to_owned());

    let cs = state_changes_to_change_set(&sc);
    assert_eq!(cs.renamed_buffers.len(), 1);
    assert_eq!(cs.renamed_buffers[0].0, buf);
    assert_eq!(cs.renamed_buffers[0].1, "new_name");
}

#[test]
fn test_has_changes_consistency() {
    // When common flags are set in StateChanges, ChangeSet.has_changes() should agree
    let mut sc = StateChanges::new();
    sc.record_mode_change();
    let buf = BufferId::new();
    sc.record_cursor_move(buf);

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.has_changes());
    assert!(cs.cursor_moved);
    assert!(cs.mode_changed);
}

#[test]
fn test_all_flags_set() {
    let mut sc = StateChanges::new();
    let buf = BufferId::new();
    let win = WindowId::new();

    sc.record_mode_change();
    sc.record_cursor_move(buf);
    sc.record_focus_change();
    sc.record_scroll_change(win);
    sc.option_changed = true;
    sc.window_changed = true;
    sc.record_quit_requested();
    sc.record_buffer_modified(buf);
    sc.record_buffer_created(buf);
    sc.record_buffer_deleted(buf);
    sc.record_window_created(win);
    sc.record_window_closed(win);

    let cs = state_changes_to_change_set(&sc);
    assert!(cs.cursor_moved);
    assert!(cs.mode_changed);
    assert!(cs.layout_changed);
    assert!(cs.focus_changed);
    assert!(cs.scroll_changed);
    assert!(cs.options_changed);
    assert!(cs.should_quit);
    assert!(!cs.modified_buffers.is_empty());
    assert!(!cs.created_buffers.is_empty());
    assert!(!cs.deleted_buffers.is_empty());
    assert!(!cs.created_windows.is_empty());
    assert!(!cs.closed_windows.is_empty());
    assert!(!cs.scrolled_windows.is_empty());
}
