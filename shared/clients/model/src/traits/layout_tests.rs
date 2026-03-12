use super::*;

struct MockLayout {
    windows: Vec<u64>,
    focused: usize,
    next_id: u64,
}

impl MockLayout {
    fn new() -> Self {
        Self {
            windows: vec![1],
            focused: 0,
            next_id: 2,
        }
    }
}

impl Layout for MockLayout {
    fn split(&mut self, _direction: SplitDirection) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.windows.push(id);
        id
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn close(&mut self, viewport_id: u64) -> bool {
        if let Some(pos) = self.windows.iter().position(|&id| id == viewport_id) {
            self.windows.remove(pos);
            if self.focused >= self.windows.len() && !self.windows.is_empty() {
                self.focused = self.windows.len() - 1;
            }
            true
        } else {
            false
        }
    }

    fn focus(&mut self, viewport_id: u64) -> bool {
        if let Some(pos) = self.windows.iter().position(|&id| id == viewport_id) {
            self.focused = pos;
            true
        } else {
            false
        }
    }

    fn focus_direction(&mut self, direction: Direction) -> bool {
        let new_focused = match direction {
            Direction::Right | Direction::Down => {
                if self.focused + 1 < self.windows.len() {
                    self.focused + 1
                } else {
                    return false;
                }
            }
            Direction::Left | Direction::Up => {
                if self.focused > 0 {
                    self.focused - 1
                } else {
                    return false;
                }
            }
        };
        self.focused = new_focused;
        true
    }

    fn focused_viewport(&self) -> u64 {
        self.windows[self.focused]
    }

    fn to_logical(&self) -> LogicalLayout {
        if self.windows.len() == 1 {
            LogicalLayout::single(self.windows[0], self.windows[0])
        } else {
            LogicalLayout::vsplit(
                self.windows
                    .iter()
                    .map(|&id| LogicalLayout::single(id, id))
                    .collect(),
            )
        }
    }

    fn apply_layout(&mut self, layout: &LogicalLayout) {
        // Simplified: just count windows
        self.windows.clear();
        self.collect_viewports(layout);
        self.focused = 0;
    }

    fn window_count(&self) -> usize {
        self.windows.len()
    }
}

impl MockLayout {
    fn collect_viewports(&mut self, layout: &LogicalLayout) {
        match layout {
            LogicalLayout::Single { viewport_id, .. } => {
                self.windows.push(*viewport_id);
            }
            LogicalLayout::Split { children, .. } => {
                for child in children {
                    self.collect_viewports(child);
                }
            }
            LogicalLayout::Tabs { tabs, .. } => {
                for tab in tabs {
                    self.collect_viewports(tab);
                }
            }
        }
    }
}

#[test]
fn test_layout_split() {
    let mut layout = MockLayout::new();
    assert_eq!(layout.window_count(), 1);

    let new_id = layout.split(SplitDirection::Vertical);
    assert_eq!(layout.window_count(), 2);
    assert!(layout.windows.contains(&new_id));
}

#[test]
fn test_layout_close() {
    let mut layout = MockLayout::new();
    let new_id = layout.split(SplitDirection::Vertical);

    assert!(layout.close(new_id));
    assert_eq!(layout.window_count(), 1);

    assert!(!layout.close(999)); // Non-existent
}

#[test]
fn test_layout_focus() {
    let mut layout = MockLayout::new();
    let new_id = layout.split(SplitDirection::Vertical);

    assert!(layout.focus(new_id));
    assert_eq!(layout.focused_viewport(), new_id);

    assert!(!layout.focus(999)); // Non-existent
}

#[test]
fn test_layout_focus_direction() {
    let mut layout = MockLayout::new();
    layout.split(SplitDirection::Vertical);

    assert_eq!(layout.focused, 0);
    assert!(layout.focus_direction(Direction::Right));
    assert_eq!(layout.focused, 1);
    assert!(!layout.focus_direction(Direction::Right)); // At end
}

#[test]
fn test_layout_is_single() {
    let mut layout = MockLayout::new();
    assert!(layout.is_single());

    layout.split(SplitDirection::Vertical);
    assert!(!layout.is_single());
}

#[test]
fn test_layout_to_logical() {
    let layout = MockLayout::new();
    let logical = layout.to_logical();
    assert!(logical.is_leaf());
}

#[test]
fn test_layout_to_logical_multiple() {
    let mut layout = MockLayout::new();
    layout.split(SplitDirection::Vertical);
    layout.split(SplitDirection::Vertical);
    let logical = layout.to_logical();
    assert!(!logical.is_leaf());
    assert_eq!(logical.window_count(), 3);
}

#[test]
fn test_layout_apply_layout() {
    let mut layout = MockLayout::new();
    let logical = LogicalLayout::vsplit(vec![
        LogicalLayout::single(10, 10),
        LogicalLayout::single(20, 20),
        LogicalLayout::single(30, 30),
    ]);
    layout.apply_layout(&logical);
    assert_eq!(layout.window_count(), 3);
    assert_eq!(layout.focused, 0);
    assert_eq!(layout.windows, vec![10, 20, 30]);
}

#[test]
fn test_layout_apply_layout_tabs() {
    let mut layout = MockLayout::new();
    let logical =
        LogicalLayout::tabs(vec![LogicalLayout::single(10, 10), LogicalLayout::single(20, 20)], 0);
    layout.apply_layout(&logical);
    assert_eq!(layout.window_count(), 2);
}

#[test]
fn test_layout_focus_direction_left() {
    let mut layout = MockLayout::new();
    layout.split(SplitDirection::Vertical);

    // Move to second window
    assert!(layout.focus_direction(Direction::Right));
    assert_eq!(layout.focused, 1);

    // Move back left
    assert!(layout.focus_direction(Direction::Left));
    assert_eq!(layout.focused, 0);

    // Can't go further left
    assert!(!layout.focus_direction(Direction::Left));
    assert_eq!(layout.focused, 0);
}

#[test]
fn test_layout_focus_direction_up() {
    let mut layout = MockLayout::new();
    layout.split(SplitDirection::Horizontal);

    // Move down
    assert!(layout.focus_direction(Direction::Down));
    assert_eq!(layout.focused, 1);

    // Move up
    assert!(layout.focus_direction(Direction::Up));
    assert_eq!(layout.focused, 0);

    // Can't go further up
    assert!(!layout.focus_direction(Direction::Up));
    assert_eq!(layout.focused, 0);
}

#[test]
fn test_layout_close_adjusts_focused() {
    let mut layout = MockLayout::new();
    let id2 = layout.split(SplitDirection::Vertical);
    let id3 = layout.split(SplitDirection::Vertical);

    // Focus the last window
    assert!(layout.focus(id3));
    assert_eq!(layout.focused, 2);

    // Close the last window - focused should adjust
    assert!(layout.close(id3));
    assert_eq!(layout.focused, 1);
    assert_eq!(layout.focused_viewport(), id2);
}

#[test]
fn test_layout_close_middle_window() {
    let mut layout = MockLayout::new();
    let id2 = layout.split(SplitDirection::Vertical);
    let id3 = layout.split(SplitDirection::Vertical);

    // Focus the last window
    assert!(layout.focus(id3));
    assert_eq!(layout.focused, 2);

    // Close the middle window
    assert!(layout.close(id2));
    // focused was 2, after removing index 1, the window at index 2 becomes index 1
    // but focused (2) >= windows.len() (2), so it adjusts to len-1 = 1
    assert_eq!(layout.window_count(), 2);
}
