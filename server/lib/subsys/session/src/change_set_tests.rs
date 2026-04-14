use reovim_kernel::api::v1::{BufferId, WindowId};

use super::ChangeSet;

#[test]
fn empty_changeset_has_no_changes() {
    let cs = ChangeSet::new();
    assert!(!cs.has_changes());
}

#[test]
fn default_changeset_has_no_changes() {
    let cs = ChangeSet::default();
    assert!(!cs.has_changes());
}

#[test]
fn record_buffer_modified() {
    let mut cs = ChangeSet::new();
    let id = BufferId::from_raw(1);
    cs.record_buffer_modified(id);
    assert!(cs.has_changes());
    assert_eq!(cs.modified_buffers, vec![id]);
}

#[test]
fn record_buffer_created() {
    let mut cs = ChangeSet::new();
    let id = BufferId::from_raw(2);
    cs.record_buffer_created(id);
    assert!(cs.has_changes());
    assert_eq!(cs.created_buffers, vec![id]);
}

#[test]
fn record_buffer_deleted() {
    let mut cs = ChangeSet::new();
    let id = BufferId::from_raw(3);
    cs.record_buffer_deleted(id);
    assert!(cs.has_changes());
    assert_eq!(cs.deleted_buffers, vec![id]);
}

#[test]
fn record_buffer_closed() {
    let mut cs = ChangeSet::new();
    let id = BufferId::from_raw(4);
    cs.record_buffer_closed(id);
    assert!(cs.has_changes());
    assert_eq!(cs.closed_buffers, vec![id]);
}

#[test]
fn record_cursor_move() {
    let mut cs = ChangeSet::new();
    cs.record_cursor_move();
    assert!(cs.has_changes());
    assert!(cs.cursor_moved);
}

#[test]
fn record_mode_change() {
    let mut cs = ChangeSet::new();
    cs.record_mode_change();
    assert!(cs.has_changes());
    assert!(cs.mode_changed);
}

#[test]
fn record_layout_change() {
    let mut cs = ChangeSet::new();
    cs.record_layout_change();
    assert!(cs.has_changes());
    assert!(cs.layout_changed);
}

#[test]
fn record_window_created_sets_layout_changed() {
    let mut cs = ChangeSet::new();
    let id = WindowId::from_raw(1);
    cs.record_window_created(id);
    assert!(cs.has_changes());
    assert!(cs.layout_changed);
    assert_eq!(cs.created_windows, vec![id]);
}

#[test]
fn record_window_closed_sets_layout_changed() {
    let mut cs = ChangeSet::new();
    let id = WindowId::from_raw(2);
    cs.record_window_closed(id);
    assert!(cs.has_changes());
    assert!(cs.layout_changed);
    assert_eq!(cs.closed_windows, vec![id]);
}

#[test]
fn record_focus_change() {
    let mut cs = ChangeSet::new();
    cs.record_focus_change();
    assert!(cs.has_changes());
    assert!(cs.focus_changed);
}

#[test]
fn record_scroll_change() {
    let mut cs = ChangeSet::new();
    let id = WindowId::from_raw(3);
    cs.record_scroll_change(id);
    assert!(cs.has_changes());
    assert!(cs.scroll_changed);
    assert_eq!(cs.scrolled_windows, vec![id]);
}

#[test]
fn record_options_change() {
    let mut cs = ChangeSet::new();
    cs.record_options_change();
    assert!(cs.has_changes());
    assert!(cs.options_changed);
}

#[test]
fn record_quit() {
    let mut cs = ChangeSet::new();
    cs.record_quit();
    assert!(cs.has_changes());
    assert!(cs.should_quit);
}

#[test]
fn record_detach() {
    let mut cs = ChangeSet::new();
    cs.record_detach();
    assert!(cs.has_changes());
    assert!(cs.should_detach);
}

#[test]
fn merge_boolean_flags_are_ored() {
    let mut a = ChangeSet::new();
    a.record_cursor_move();

    let mut b = ChangeSet::new();
    b.record_mode_change();
    b.record_quit();

    a.merge(b);
    assert!(a.cursor_moved);
    assert!(a.mode_changed);
    assert!(a.should_quit);
}

#[test]
fn merge_id_lists_are_extended() {
    let mut a = ChangeSet::new();
    let buf1 = BufferId::from_raw(1);
    let buf2 = BufferId::from_raw(2);
    a.record_buffer_modified(buf1);

    let mut b = ChangeSet::new();
    b.record_buffer_modified(buf2);

    a.merge(b);
    assert_eq!(a.modified_buffers, vec![buf1, buf2]);
}

#[test]
fn merge_with_duplicate_ids() {
    let mut a = ChangeSet::new();
    let buf = BufferId::from_raw(1);
    a.record_buffer_modified(buf);

    let mut b = ChangeSet::new();
    b.record_buffer_modified(buf);

    a.merge(b);
    // No deduplication — documented behavior
    assert_eq!(a.modified_buffers, vec![buf, buf]);
}

#[test]
fn merge_empty_into_populated() {
    let mut a = ChangeSet::new();
    a.record_cursor_move();
    a.record_buffer_modified(BufferId::from_raw(1));

    let b = ChangeSet::new();
    a.merge(b);
    assert!(a.cursor_moved);
    assert_eq!(a.modified_buffers.len(), 1);
}

#[test]
fn merge_populated_into_empty() {
    let mut a = ChangeSet::new();

    let mut b = ChangeSet::new();
    b.record_mode_change();
    b.record_buffer_created(BufferId::from_raw(5));

    a.merge(b);
    assert!(a.mode_changed);
    assert_eq!(a.created_buffers, vec![BufferId::from_raw(5)]);
}

#[test]
fn merge_window_lists() {
    let mut a = ChangeSet::new();
    a.record_window_created(WindowId::from_raw(1));
    a.record_scroll_change(WindowId::from_raw(2));

    let mut b = ChangeSet::new();
    b.record_window_closed(WindowId::from_raw(3));
    b.record_scroll_change(WindowId::from_raw(4));

    a.merge(b);
    assert_eq!(a.created_windows, vec![WindowId::from_raw(1)]);
    assert_eq!(a.closed_windows, vec![WindowId::from_raw(3)]);
    assert_eq!(a.scrolled_windows, vec![WindowId::from_raw(2), WindowId::from_raw(4)]);
}

#[test]
fn has_changes_with_only_scrolled_windows() {
    let mut cs = ChangeSet::new();
    cs.scrolled_windows.push(WindowId::from_raw(1));
    assert!(cs.has_changes());
}

#[test]
fn clone_preserves_all_fields() {
    let mut cs = ChangeSet::new();
    cs.record_buffer_modified(BufferId::from_raw(1));
    cs.record_cursor_move();
    cs.record_quit();

    let cloned = cs.clone();
    assert_eq!(cloned.modified_buffers, vec![BufferId::from_raw(1)]);
    assert!(cloned.cursor_moved);
    assert!(cloned.should_quit);
}

#[test]
fn debug_format() {
    let cs = ChangeSet::new();
    let debug = format!("{cs:?}");
    assert!(debug.contains("ChangeSet"));
}
