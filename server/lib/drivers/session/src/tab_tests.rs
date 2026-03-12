use super::*;

#[test]
fn test_tab_page_new() {
    let tab = TabPage::new("main");
    assert_eq!(tab.label(), "main");
    assert!(tab.compositor().is_none());
    assert!(tab.windows().is_empty());
}

#[test]
fn test_tab_page_with_id() {
    let id = TabId::new();
    let tab = TabPage::with_id(id, "test");
    assert_eq!(tab.id(), id);
    assert_eq!(tab.label(), "test");
}

#[test]
fn test_tab_page_set_label() {
    let mut tab = TabPage::new("old");
    tab.set_label("new");
    assert_eq!(tab.label(), "new");
}

#[test]
fn test_tab_page_compositor_mut() {
    let mut tab = TabPage::new("test");
    assert!(tab.compositor_mut().is_none());
}

#[test]
fn test_tab_page_set_compositor() {
    let mut tab = TabPage::new("test");
    tab.set_compositor(None);
    assert!(tab.compositor().is_none());
}

#[test]
fn test_tab_page_windows_mut() {
    let mut tab = TabPage::new("test");
    assert!(tab.windows_mut().is_empty());
}

#[test]
fn test_tab_page_debug() {
    let tab = TabPage::new("test");
    let debug = format!("{tab:?}");
    assert!(debug.contains("TabPage"));
}

#[test]
fn test_tab_page_set_new() {
    let set = TabPageSet::new();
    assert_eq!(set.tab_count(), 1);
    assert_eq!(set.active_index(), 0);
    assert_eq!(set.active_tab().label(), "1");
}

#[test]
fn test_tab_page_set_default() {
    let set = TabPageSet::default();
    assert_eq!(set.tab_count(), 1);
}

#[test]
fn test_tab_page_set_new_tab() {
    let mut set = TabPageSet::new();
    let id1 = set.active_tab_id();
    let id2 = set.new_tab();
    assert_ne!(id1, id2);
    assert_eq!(set.tab_count(), 2);
    assert_eq!(set.active_index(), 1);
    assert_eq!(set.active_tab_id(), id2);
}

#[test]
fn test_tab_page_set_close_tab() {
    let mut set = TabPageSet::new();
    // Cannot close the only tab
    assert!(!set.close_tab());
    assert_eq!(set.tab_count(), 1);

    // Add a tab and close it
    set.new_tab();
    assert_eq!(set.tab_count(), 2);
    assert!(set.close_tab());
    assert_eq!(set.tab_count(), 1);
}

#[test]
fn test_tab_page_set_close_last_index() {
    let mut set = TabPageSet::new();
    set.new_tab();
    set.new_tab();
    // Active is now index 2 (last tab)
    assert_eq!(set.active_index(), 2);
    assert!(set.close_tab());
    // Should move to index 1 (previous)
    assert_eq!(set.active_index(), 1);
}

#[test]
fn test_tab_page_set_close_middle_index() {
    let mut set = TabPageSet::new();
    let id1 = set.active_tab_id();
    set.new_tab();
    set.new_tab();
    // Go back to middle (index 1)
    set.goto_tab(1);
    assert_eq!(set.active_index(), 1);
    assert!(set.close_tab());
    // After closing index 1, index 1 now points to the former index 2
    assert_eq!(set.tab_count(), 2);
    assert_eq!(set.active_index(), 1);
    // First tab should still be id1
    assert_eq!(set.tabs()[0].id(), id1);
}

#[test]
fn test_tab_page_set_next_tab() {
    let mut set = TabPageSet::new();
    let id1 = set.active_tab_id();
    let id2 = set.new_tab();

    // Currently at tab 2, next should wrap to tab 1
    let next = set.next_tab();
    assert_eq!(next, id1);

    // Next again should go to tab 2
    let next = set.next_tab();
    assert_eq!(next, id2);
}

#[test]
fn test_tab_page_set_prev_tab() {
    let mut set = TabPageSet::new();
    let id1 = set.active_tab_id();
    let id2 = set.new_tab();

    // Currently at tab 2, prev should go to tab 1
    let prev = set.prev_tab();
    assert_eq!(prev, id1);

    // Prev again should wrap to tab 2
    let prev = set.prev_tab();
    assert_eq!(prev, id2);
}

#[test]
fn test_tab_page_set_goto_tab() {
    let mut set = TabPageSet::new();
    let id1 = set.active_tab_id();
    set.new_tab();
    let id3 = set.new_tab();

    // Go to first tab
    let result = set.goto_tab(0);
    assert_eq!(result, Some(id1));
    assert_eq!(set.active_index(), 0);

    // Go to last tab
    let result = set.goto_tab(2);
    assert_eq!(result, Some(id3));
    assert_eq!(set.active_index(), 2);

    // Out of bounds
    let result = set.goto_tab(10);
    assert!(result.is_none());
    // Active index unchanged
    assert_eq!(set.active_index(), 2);
}

#[test]
fn test_tab_page_set_find_tab() {
    let mut set = TabPageSet::new();
    let id1 = set.active_tab_id();
    let id2 = set.new_tab();

    let (idx, tab) = set.find_tab(id1).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(tab.id(), id1);

    let (idx, tab) = set.find_tab(id2).unwrap();
    assert_eq!(idx, 1);
    assert_eq!(tab.id(), id2);

    // Non-existent ID
    let fake_id = TabId::from_raw(9999);
    assert!(set.find_tab(fake_id).is_none());
}

#[test]
fn test_tab_page_set_tab_info() {
    let mut set = TabPageSet::new();
    set.new_tab();

    let info = set.tab_info();
    assert_eq!(info.len(), 2);
    assert!(!info[0].2); // First tab not active
    assert!(info[1].2); // Second tab active (we just created it)
}

#[test]
fn test_tab_page_set_active_windows() {
    let set = TabPageSet::new();
    assert!(set.active_windows().is_empty());
}

#[test]
fn test_tab_page_set_active_windows_mut() {
    let mut set = TabPageSet::new();
    assert!(set.active_windows_mut().is_empty());
}

#[test]
fn test_tab_page_set_active_compositor() {
    let set = TabPageSet::new();
    assert!(set.active_compositor().is_none());
}

#[test]
fn test_tab_page_set_active_compositor_mut() {
    let mut set = TabPageSet::new();
    assert!(set.active_compositor_mut().is_none());
}

#[test]
fn test_tab_page_set_tabs_slice() {
    let mut set = TabPageSet::new();
    set.new_tab();
    assert_eq!(set.tabs().len(), 2);
}

#[test]
fn test_tab_page_set_active_tab_mut() {
    let mut set = TabPageSet::new();
    set.active_tab_mut().set_label("renamed");
    assert_eq!(set.active_tab().label(), "renamed");
}

#[test]
fn test_tab_page_set_debug() {
    let set = TabPageSet::new();
    let debug = format!("{set:?}");
    assert!(debug.contains("TabPageSet"));
}

#[test]
fn test_tab_page_clone() {
    let tab = TabPage::new("cloned");
    #[allow(clippy::redundant_clone)]
    let cloned = tab.clone();
    assert_eq!(cloned.id(), tab.id());
    assert_eq!(cloned.label(), "cloned");
    assert!(cloned.compositor().is_none());
    assert!(cloned.windows().is_empty());
}

#[test]
fn test_tab_page_set_single_tab_next_wraps() {
    let mut set = TabPageSet::new();
    let id = set.active_tab_id();
    // With one tab, next should return the same tab
    let next = set.next_tab();
    assert_eq!(next, id);
}

#[test]
fn test_tab_page_set_single_tab_prev_wraps() {
    let mut set = TabPageSet::new();
    let id = set.active_tab_id();
    // With one tab, prev should return the same tab
    let prev = set.prev_tab();
    assert_eq!(prev, id);
}

#[test]
fn test_tab_page_set_new_tab_label_increments() {
    let mut set = TabPageSet::new();
    set.new_tab();
    set.new_tab();
    assert_eq!(set.tabs()[0].label(), "1");
    assert_eq!(set.tabs()[1].label(), "2");
    assert_eq!(set.tabs()[2].label(), "3");
}

#[test]
fn test_tab_page_set_new_tab_inserts_after_active() {
    let mut set = TabPageSet::new();
    let id1 = set.active_tab_id();
    let id2 = set.new_tab();
    // Go back to first tab
    set.goto_tab(0);
    // Create tab after first — should insert at index 1
    let id3 = set.new_tab();
    assert_eq!(set.active_index(), 1);
    assert_eq!(set.active_tab_id(), id3);
    // Order should be: id1, id3, id2
    assert_eq!(set.tabs()[0].id(), id1);
    assert_eq!(set.tabs()[1].id(), id3);
    assert_eq!(set.tabs()[2].id(), id2);
}
