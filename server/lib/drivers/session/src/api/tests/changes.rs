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

// === Quit signal tests (#547) ===

#[test]
fn test_quit_not_in_has_changes() {
    let mut changes = StateChanges::new();
    changes.record_quit_requested();
    // should_quit is a lifecycle signal, NOT a state change
    assert!(!changes.has_changes());
    assert!(changes.should_quit);
}

#[test]
fn test_quit_default_false() {
    let changes = StateChanges::new();
    assert!(!changes.should_quit);
}

#[test]
fn test_merge_quit_signal() {
    let mut a = StateChanges::new();
    let mut b = StateChanges::new();
    b.record_quit_requested();

    a.merge(b);
    assert!(a.should_quit);
}

#[test]
fn test_merge_quit_both_set() {
    let mut a = StateChanges::new();
    a.record_quit_requested();
    let mut b = StateChanges::new();
    b.record_quit_requested();

    a.merge(b);
    assert!(a.should_quit);
}

#[test]
fn test_merge_quit_preserves_existing() {
    let mut a = StateChanges::new();
    a.record_quit_requested();
    let b = StateChanges::new();

    a.merge(b);
    assert!(a.should_quit);
}
