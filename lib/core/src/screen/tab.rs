//! Tab page management for vim-style tabs

use super::split::{find_adjacent_window, NavigateDirection, SplitDirection, SplitNode, WindowLayout, WindowRect};

#[derive(Debug)]
pub struct TabPage {
    pub id: usize,
    pub root: SplitNode,
    pub active_window_id: usize,
    pub label: Option<String>,
}

impl TabPage {
    #[must_use]
    pub const fn new(id: usize, window_id: usize) -> Self {
        Self {
            id,
            root: SplitNode::leaf(window_id),
            active_window_id: window_id,
            label: None,
        }
    }

    #[must_use]
    pub fn window_ids(&self) -> Vec<usize> {
        self.root.window_ids()
    }

    #[must_use]
    pub fn window_count(&self) -> usize {
        self.root.window_count()
    }

    pub fn split(&mut self, new_window_id: usize, direction: SplitDirection) {
        self.root.split_window(self.active_window_id, new_window_id, direction);
        self.active_window_id = new_window_id;
    }

    pub fn close_window(&mut self, window_id: usize) -> bool {
        if self.root.window_count() == 1 {
            return true;
        }

        let next_focus = if window_id == self.active_window_id {
            self.find_next_focus(window_id)
        } else {
            None
        };

        self.root.remove_window(window_id);

        if let Some(next) = next_focus {
            self.active_window_id = next;
        }

        false
    }

    fn find_next_focus(&self, closing_id: usize) -> Option<usize> {
        self.root
            .window_ids()
            .into_iter()
            .find(|&id| id != closing_id)
    }

    pub fn navigate(&mut self, direction: NavigateDirection, layouts: &[WindowLayout]) {
        if let Some(next_id) = find_adjacent_window(self.active_window_id, direction, layouts) {
            self.active_window_id = next_id;
        }
    }

    #[must_use]
    pub fn calculate_layouts(&self, rect: WindowRect) -> Vec<WindowLayout> {
        let mut layouts = Vec::new();
        self.root.calculate_layouts(rect, &mut layouts);
        layouts
    }

    pub fn equalize(&mut self) {
        self.root.equalize();
    }

    pub fn adjust_ratio(&mut self, delta: f32) {
        self.root.adjust_ratio(self.active_window_id, delta);
    }
}

#[derive(Debug)]
pub struct TabManager {
    tabs: Vec<TabPage>,
    active_tab_index: usize,
    next_tab_id: usize,
}

impl TabManager {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_index: 0,
            next_tab_id: 1,
        }
    }

    pub fn init_with_window(&mut self, window_id: usize) {
        let tab = TabPage::new(self.next_tab_id, window_id);
        self.next_tab_id += 1;
        self.tabs.push(tab);
    }

    #[must_use]
    pub fn active_tab(&self) -> Option<&TabPage> {
        self.tabs.get(self.active_tab_index)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut TabPage> {
        self.tabs.get_mut(self.active_tab_index)
    }

    #[must_use]
    pub fn active_window_id(&self) -> Option<usize> {
        self.active_tab().map(|t| t.active_window_id)
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[must_use]
    pub const fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    #[must_use]
    pub fn tabs(&self) -> &[TabPage] {
        &self.tabs
    }

    pub fn new_tab(&mut self, window_id: usize) -> usize {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = TabPage::new(tab_id, window_id);
        self.tabs.insert(self.active_tab_index + 1, tab);
        self.active_tab_index += 1;
        tab_id
    }

    pub fn close_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.remove(self.active_tab_index);
        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        true
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn goto_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab_index = index;
        }
    }

    #[must_use]
    pub fn tab_info(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| TabInfo {
                id: tab.id,
                label: tab.label.clone().unwrap_or_else(|| format!("Tab {}", i + 1)),
                is_active: i == self.active_tab_index,
            })
            .collect()
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TabInfo {
    pub id: usize,
    pub label: String,
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_page_new() {
        let tab = TabPage::new(1, 10);
        assert_eq!(tab.id, 1);
        assert_eq!(tab.active_window_id, 10);
        assert_eq!(tab.window_count(), 1);
    }

    #[test]
    fn test_tab_page_split() {
        let mut tab = TabPage::new(1, 10);
        tab.split(20, SplitDirection::Vertical);
        assert_eq!(tab.window_count(), 2);
        assert_eq!(tab.active_window_id, 20);
    }

    #[test]
    fn test_tab_page_close_window() {
        let mut tab = TabPage::new(1, 10);
        tab.split(20, SplitDirection::Vertical);

        let is_last = tab.close_window(20);
        assert!(!is_last);
        assert_eq!(tab.window_count(), 1);
        assert_eq!(tab.active_window_id, 10);
    }

    #[test]
    fn test_tab_page_close_last_window() {
        let mut tab = TabPage::new(1, 10);
        let is_last = tab.close_window(10);
        assert!(is_last);
    }

    #[test]
    fn test_tab_manager_init() {
        let mut manager = TabManager::new();
        manager.init_with_window(10);
        assert_eq!(manager.tab_count(), 1);
        assert_eq!(manager.active_window_id(), Some(10));
    }

    #[test]
    fn test_tab_manager_new_tab() {
        let mut manager = TabManager::new();
        manager.init_with_window(10);
        manager.new_tab(20);

        assert_eq!(manager.tab_count(), 2);
        assert_eq!(manager.active_tab_index(), 1);
        assert_eq!(manager.active_window_id(), Some(20));
    }

    #[test]
    fn test_tab_manager_navigation() {
        let mut manager = TabManager::new();
        manager.init_with_window(10);
        manager.new_tab(20);
        manager.new_tab(30);

        assert_eq!(manager.active_tab_index(), 2);

        manager.prev_tab();
        assert_eq!(manager.active_tab_index(), 1);

        manager.next_tab();
        assert_eq!(manager.active_tab_index(), 2);

        manager.goto_tab(0);
        assert_eq!(manager.active_tab_index(), 0);
    }

    #[test]
    fn test_tab_manager_close_tab() {
        let mut manager = TabManager::new();
        manager.init_with_window(10);
        manager.new_tab(20);

        assert!(manager.close_tab());
        assert_eq!(manager.tab_count(), 1);

        assert!(!manager.close_tab());
        assert_eq!(manager.tab_count(), 1);
    }
}
