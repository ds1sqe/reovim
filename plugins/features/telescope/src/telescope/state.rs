//! Telescope state management

use {
    super::item::TelescopeItem,
    reovim_core::overlay::{OverlayBounds, OverlayGeometry, Scrollable, Selectable},
};

/// Preview content for the selected item
#[derive(Debug, Clone, Default)]
pub struct PreviewContent {
    /// Lines of the preview content
    pub lines: Vec<String>,
    /// Line to highlight (0-indexed)
    pub highlight_line: Option<usize>,
    /// File extension for syntax highlighting
    pub syntax: Option<String>,
}

impl PreviewContent {
    /// Create a new preview content
    #[must_use]
    pub const fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            highlight_line: None,
            syntax: None,
        }
    }

    /// Set the line to highlight
    #[must_use]
    pub const fn with_highlight_line(mut self, line: usize) -> Self {
        self.highlight_line = Some(line);
        self
    }

    /// Set the syntax type
    #[must_use]
    pub fn with_syntax(mut self, syntax: impl Into<String>) -> Self {
        self.syntax = Some(syntax.into());
        self
    }
}

/// Layout information for telescope UI
#[derive(Debug, Clone, Default)]
pub struct TelescopeLayout {
    /// X position of the panel
    pub x: u16,
    /// Y position of the panel
    pub y: u16,
    /// Width of the results panel
    pub width: u16,
    /// Height of the panel
    pub height: u16,
    /// Width of the preview panel (if enabled)
    pub preview_width: Option<u16>,
    /// Maximum visible items
    pub visible_items: usize,
}

/// State of the telescope fuzzy finder
#[derive(Debug, Clone, Default)]
pub struct TelescopeState {
    /// Whether telescope is currently active/visible
    pub active: bool,
    /// Current search query
    pub query: String,
    /// Cursor position in the query
    pub cursor_pos: usize,
    /// All items from the picker (unfiltered)
    pub all_items: Vec<TelescopeItem>,
    /// Current list of items (filtered/sorted)
    pub items: Vec<TelescopeItem>,
    /// Currently selected item index
    pub selected_index: usize,
    /// Scroll offset for long lists
    pub scroll_offset: usize,
    /// Name of the current picker
    pub picker_name: String,
    /// Title to display
    pub title: String,
    /// Prompt string
    pub prompt: String,
    /// Preview content (if available)
    pub preview: Option<PreviewContent>,
    /// Layout configuration
    pub layout: TelescopeLayout,
    /// Whether preview is enabled
    pub preview_enabled: bool,
}

impl TelescopeState {
    /// Create a new empty telescope state
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            cursor_pos: 0,
            all_items: Vec::new(),
            items: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            picker_name: String::new(),
            title: String::new(),
            prompt: "> ".to_string(),
            preview: None,
            layout: TelescopeLayout::default(),
            preview_enabled: true,
        }
    }

    /// Open telescope with a picker
    pub fn open(&mut self, picker_name: &str, title: &str, prompt: &str) {
        self.active = true;
        self.query.clear();
        self.cursor_pos = 0;
        self.all_items.clear();
        self.items.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.picker_name = picker_name.to_string();
        self.title = title.to_string();
        self.prompt = prompt.to_string();
        self.preview = None;
    }

    /// Close telescope
    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.cursor_pos = 0;
        self.all_items.clear();
        self.items.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.picker_name.clear();
        self.preview = None;
    }

    /// Update items from search results (initial load - stores in both `all_items` and items)
    pub fn update_items(&mut self, items: Vec<TelescopeItem>) {
        self.all_items = items.clone();
        self.items = items;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.ensure_selected_visible();
    }

    /// Update filtered items only (for filtering - keeps `all_items` unchanged)
    pub fn update_filtered_items(&mut self, items: Vec<TelescopeItem>) {
        self.items = items;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.ensure_selected_visible();
    }

    /// Insert a character at cursor position
    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Delete character before cursor
    pub fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            // Find the previous char boundary
            let prev_pos = self.query[..self.cursor_pos]
                .char_indices()
                .last()
                .map_or(0, |(i, _)| i);
            self.query.remove(prev_pos);
            self.cursor_pos = prev_pos;
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.query[..self.cursor_pos]
                .char_indices()
                .last()
                .map_or(0, |(i, _)| i);
        }
    }

    /// Move cursor right
    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.query.len() {
            let query_len = self.query.len();
            self.cursor_pos = self.query[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map_or(query_len, |(i, _)| self.cursor_pos + i);
        }
    }

    /// Move cursor to start
    pub const fn cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end
    #[allow(clippy::missing_const_for_fn)] // String::len is not const-stable
    pub fn cursor_end(&mut self) {
        self.cursor_pos = self.query.len();
    }

    /// Select next item
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.items.len();
            self.ensure_selected_visible();
        }
    }

    /// Select previous item
    pub fn select_prev(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.items.len() - 1);
            self.ensure_selected_visible();
        }
    }

    /// Page down
    pub fn page_down(&mut self) {
        if !self.items.is_empty() {
            let page_size = self.layout.visible_items.max(1);
            self.selected_index = (self.selected_index + page_size).min(self.items.len() - 1);
            self.ensure_selected_visible();
        }
    }

    /// Page up
    pub fn page_up(&mut self) {
        if !self.items.is_empty() {
            let page_size = self.layout.visible_items.max(1);
            self.selected_index = self.selected_index.saturating_sub(page_size);
            self.ensure_selected_visible();
        }
    }

    /// Move to first item
    pub fn move_to_first(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = 0;
            self.ensure_selected_visible();
        }
    }

    /// Move to last item
    pub fn move_to_last(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = self.items.len() - 1;
            self.ensure_selected_visible();
        }
    }

    /// Get currently selected item
    #[must_use]
    pub fn selected_item(&self) -> Option<&TelescopeItem> {
        self.items.get(self.selected_index)
    }

    /// Check if telescope is active and visible
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.active
    }

    /// Ensure selected item is visible in the viewport
    fn ensure_selected_visible(&mut self) {
        let visible = self.layout.visible_items.max(1);

        // Scroll down if selected is below visible area
        if self.selected_index >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_index - visible + 1;
        }

        // Scroll up if selected is above visible area
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    /// Get visible items slice
    #[must_use]
    pub fn visible_items(&self) -> &[TelescopeItem] {
        let start = self.scroll_offset;
        let end = (start + self.layout.visible_items).min(self.items.len());
        &self.items[start..end]
    }

    /// Update preview content
    pub fn set_preview(&mut self, content: Option<PreviewContent>) {
        self.preview = content;
    }

    /// Calculate layout based on screen dimensions
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn calculate_layout(&mut self, screen_width: u16, screen_height: u16) {
        // 80% width, 70% height
        let total_width = (f32::from(screen_width) * 0.8) as u16;
        let height = (f32::from(screen_height) * 0.7) as u16;

        let x = (screen_width - total_width) / 2;
        let y = (screen_height - height) / 2;

        if self.preview_enabled {
            // 40% for results, 60% for preview
            let results_width = (f32::from(total_width) * 0.4) as u16;
            let preview_width = total_width - results_width - 1; // -1 for separator

            self.layout = TelescopeLayout {
                x,
                y,
                width: results_width,
                height,
                preview_width: Some(preview_width),
                visible_items: usize::from(height.saturating_sub(4)), // -4 for borders and prompt
            };
        } else {
            self.layout = TelescopeLayout {
                x,
                y,
                width: total_width,
                height,
                preview_width: None,
                visible_items: usize::from(height.saturating_sub(4)),
            };
        }
    }
}

// === Overlay Trait Implementations ===

impl Selectable for TelescopeState {
    fn item_count(&self) -> usize {
        self.items.len()
    }

    fn selected_index(&self) -> usize {
        self.selected_index
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected_index = index.min(self.items.len().saturating_sub(1));
    }
}

impl Scrollable for TelescopeState {
    fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    fn visible_item_count(&self) -> usize {
        self.layout.visible_items.max(1)
    }
}

impl OverlayGeometry for TelescopeState {
    /// Compute bounds for the telescope overlay
    ///
    /// Uses 80% width and 70% height, centered on screen.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn compute_bounds(&self, screen_width: u16, screen_height: u16) -> OverlayBounds {
        // 80% width, 70% height
        let total_width = (f32::from(screen_width) * 0.8) as u16;
        let height = (f32::from(screen_height) * 0.7) as u16;

        let x = (screen_width - total_width) / 2;
        let y = (screen_height - height) / 2;

        OverlayBounds::new(x, y, total_width, height)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::telescope::item::TelescopeData, std::path::PathBuf};

    fn sample_items() -> Vec<TelescopeItem> {
        vec![
            TelescopeItem::new(
                "1",
                "file1.rs",
                TelescopeData::FilePath(PathBuf::from("file1.rs")),
                "files",
            ),
            TelescopeItem::new(
                "2",
                "file2.rs",
                TelescopeData::FilePath(PathBuf::from("file2.rs")),
                "files",
            ),
            TelescopeItem::new(
                "3",
                "file3.rs",
                TelescopeData::FilePath(PathBuf::from("file3.rs")),
                "files",
            ),
        ]
    }

    #[test]
    fn test_new_state() {
        let state = TelescopeState::new();
        assert!(!state.active);
        assert!(state.query.is_empty());
        assert_eq!(state.cursor_pos, 0);
        assert!(state.items.is_empty());
    }

    #[test]
    fn test_open_close() {
        let mut state = TelescopeState::new();
        state.open("files", "Find Files", "Files> ");

        assert!(state.active);
        assert_eq!(state.picker_name, "files");
        assert_eq!(state.title, "Find Files");
        assert_eq!(state.prompt, "Files> ");

        state.close();
        assert!(!state.active);
        assert!(state.picker_name.is_empty());
    }

    #[test]
    fn test_insert_delete() {
        let mut state = TelescopeState::new();
        state.open("files", "Test", "> ");

        state.insert_char('h');
        state.insert_char('e');
        state.insert_char('l');
        state.insert_char('l');
        state.insert_char('o');

        assert_eq!(state.query, "hello");
        assert_eq!(state.cursor_pos, 5);

        state.delete_char();
        assert_eq!(state.query, "hell");
        assert_eq!(state.cursor_pos, 4);
    }

    #[test]
    fn test_cursor_movement() {
        let mut state = TelescopeState::new();
        state.open("files", "Test", "> ");
        state.query = "hello".to_string();
        state.cursor_pos = 3;

        state.cursor_left();
        assert_eq!(state.cursor_pos, 2);

        state.cursor_right();
        assert_eq!(state.cursor_pos, 3);

        state.cursor_home();
        assert_eq!(state.cursor_pos, 0);

        state.cursor_end();
        assert_eq!(state.cursor_pos, 5);
    }

    #[test]
    fn test_selection() {
        let mut state = TelescopeState::new();
        state.open("files", "Test", "> ");
        state.layout.visible_items = 10;
        state.update_items(sample_items());

        assert_eq!(state.selected_index, 0);

        state.select_next();
        assert_eq!(state.selected_index, 1);

        state.select_next();
        assert_eq!(state.selected_index, 2);

        state.select_next(); // Wraps
        assert_eq!(state.selected_index, 0);

        state.select_prev(); // Wraps back
        assert_eq!(state.selected_index, 2);
    }
}
