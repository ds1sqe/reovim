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
    let buf = BufferId::new();

    // These text-domain fields should NOT appear in ChangeSet
    sc.selection_changed = true;
    sc.record_selection_change(buf);
    sc.buffer_modified = true;
    // text_buffer_edits, byte_edits have no ChangeSet equivalent

    let cs = state_changes_to_change_set(&sc);
    // ChangeSet has no selection field — only cursor_moved (which is false)
    assert!(!cs.cursor_moved);
    // buffer_modified only matters if modified_buffers is populated
    assert!(cs.modified_buffers.is_empty());
}

#[test]
fn test_deferred_fields_not_mapped() {
    let mut sc = StateChanges::new();
    let buf = BufferId::new();

    // These are mechanism fields deferred to sub-plan 05
    sc.record_buffer_renamed(buf, "new_name".to_owned());
    sc.record_presence_change(1);
    sc.record_extension_change("cmdline".to_owned());

    let cs = state_changes_to_change_set(&sc);
    // None of these should create changes in ChangeSet yet
    // ChangeSet doesn't have renamed_buffers, presence, or extension fields
    assert!(!cs.has_changes());
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
