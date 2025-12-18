//! Selection and scrolling traits with default implementations

/// Trait for overlays with selectable items
///
/// Provides default wraparound navigation behavior.
pub trait Selectable {
    /// Total number of items in the list
    fn item_count(&self) -> usize;

    /// Current selected index
    fn selected_index(&self) -> usize;

    /// Set the selected index (must be < `item_count()`)
    fn set_selected_index(&mut self, index: usize);

    /// Move selection to next item with wraparound
    fn select_next(&mut self) {
        let count = self.item_count();
        if count == 0 {
            return;
        }
        let current = self.selected_index();
        let next = if current + 1 >= count { 0 } else { current + 1 };
        self.set_selected_index(next);
    }

    /// Move selection to previous item with wraparound
    fn select_prev(&mut self) {
        let count = self.item_count();
        if count == 0 {
            return;
        }
        let current = self.selected_index();
        let prev = if current == 0 {
            count.saturating_sub(1)
        } else {
            current - 1
        };
        self.set_selected_index(prev);
    }

    /// Move selection to first item
    fn select_first(&mut self) {
        if self.item_count() > 0 {
            self.set_selected_index(0);
        }
    }

    /// Move selection to last item
    fn select_last(&mut self) {
        let count = self.item_count();
        if count > 0 {
            self.set_selected_index(count - 1);
        }
    }
}

/// Trait for overlays with scrollable content
///
/// Extends `Selectable` with viewport management and scroll tracking.
pub trait Scrollable: Selectable {
    /// Current scroll offset (first visible item index)
    fn scroll_offset(&self) -> usize;

    /// Set the scroll offset
    fn set_scroll_offset(&mut self, offset: usize);

    /// Number of items visible in the viewport
    fn visible_item_count(&self) -> usize;

    /// Ensure the selected item is visible in the viewport
    ///
    /// Adjusts scroll offset so that `selected_index` is within the visible range.
    fn ensure_selected_visible(&mut self) {
        let selected = self.selected_index();
        let offset = self.scroll_offset();
        let visible = self.visible_item_count();

        if selected < offset {
            // Selection is above viewport, scroll up
            self.set_scroll_offset(selected);
        } else if selected >= offset + visible {
            // Selection is below viewport, scroll down
            self.set_scroll_offset(selected.saturating_sub(visible.saturating_sub(1)));
        }
    }

    /// Scroll down by one page
    fn page_down(&mut self) {
        let count = self.item_count();
        if count == 0 {
            return;
        }
        let visible = self.visible_item_count();
        let current = self.selected_index();
        let new_index = (current + visible).min(count.saturating_sub(1));
        self.set_selected_index(new_index);
        self.ensure_selected_visible();
    }

    /// Scroll up by one page
    fn page_up(&mut self) {
        if self.item_count() == 0 {
            return;
        }
        let visible = self.visible_item_count();
        let current = self.selected_index();
        let new_index = current.saturating_sub(visible);
        self.set_selected_index(new_index);
        self.ensure_selected_visible();
    }

    /// Scroll down by half a page
    fn half_page_down(&mut self) {
        let count = self.item_count();
        if count == 0 {
            return;
        }
        let half = self.visible_item_count() / 2;
        let current = self.selected_index();
        let new_index = (current + half).min(count.saturating_sub(1));
        self.set_selected_index(new_index);
        self.ensure_selected_visible();
    }

    /// Scroll up by half a page
    fn half_page_up(&mut self) {
        if self.item_count() == 0 {
            return;
        }
        let half = self.visible_item_count() / 2;
        let current = self.selected_index();
        let new_index = current.saturating_sub(half);
        self.set_selected_index(new_index);
        self.ensure_selected_visible();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation
    struct TestList {
        items: Vec<String>,
        selected: usize,
        scroll: usize,
        visible: usize,
    }

    impl TestList {
        fn new(count: usize, visible: usize) -> Self {
            Self {
                items: (0..count).map(|i| format!("Item {i}")).collect(),
                selected: 0,
                scroll: 0,
                visible,
            }
        }
    }

    impl Selectable for TestList {
        fn item_count(&self) -> usize {
            self.items.len()
        }
        fn selected_index(&self) -> usize {
            self.selected
        }
        fn set_selected_index(&mut self, index: usize) {
            self.selected = index;
        }
    }

    impl Scrollable for TestList {
        fn scroll_offset(&self) -> usize {
            self.scroll
        }
        fn set_scroll_offset(&mut self, offset: usize) {
            self.scroll = offset;
        }
        fn visible_item_count(&self) -> usize {
            self.visible
        }
    }

    #[test]
    fn test_select_next_wraparound() {
        let mut list = TestList::new(3, 3);
        assert_eq!(list.selected_index(), 0);
        list.select_next();
        assert_eq!(list.selected_index(), 1);
        list.select_next();
        assert_eq!(list.selected_index(), 2);
        list.select_next(); // Should wrap to 0
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_select_prev_wraparound() {
        let mut list = TestList::new(3, 3);
        assert_eq!(list.selected_index(), 0);
        list.select_prev(); // Should wrap to 2
        assert_eq!(list.selected_index(), 2);
        list.select_prev();
        assert_eq!(list.selected_index(), 1);
    }

    #[test]
    fn test_select_first_last() {
        let mut list = TestList::new(5, 3);
        list.set_selected_index(2);
        list.select_last();
        assert_eq!(list.selected_index(), 4);
        list.select_first();
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_ensure_selected_visible_scroll_down() {
        let mut list = TestList::new(10, 3);
        list.set_selected_index(5);
        list.ensure_selected_visible();
        // Selected 5 should be visible, scroll offset should be at most 3
        assert!(list.scroll_offset() <= 5);
        assert!(list.scroll_offset() + list.visible_item_count() > 5);
    }

    #[test]
    fn test_ensure_selected_visible_scroll_up() {
        let mut list = TestList::new(10, 3);
        list.set_scroll_offset(5);
        list.set_selected_index(2);
        list.ensure_selected_visible();
        assert_eq!(list.scroll_offset(), 2);
    }

    #[test]
    fn test_page_down() {
        let mut list = TestList::new(10, 3);
        list.page_down();
        assert_eq!(list.selected_index(), 3);
        list.page_down();
        assert_eq!(list.selected_index(), 6);
        list.page_down();
        assert_eq!(list.selected_index(), 9); // Clamped to last
    }

    #[test]
    fn test_page_up() {
        let mut list = TestList::new(10, 3);
        list.set_selected_index(9);
        list.page_up();
        assert_eq!(list.selected_index(), 6);
        list.page_up();
        assert_eq!(list.selected_index(), 3);
        list.page_up();
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_empty_list() {
        let mut list = TestList::new(0, 3);
        list.select_next(); // Should not panic
        list.select_prev(); // Should not panic
        assert_eq!(list.selected_index(), 0);
    }
}
