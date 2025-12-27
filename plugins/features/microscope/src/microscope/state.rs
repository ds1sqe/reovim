//! Microscope state management

use {
    super::{
        item::MicroscopeItem,
        layout::{LayoutBounds, LayoutConfig, calculate_layout, visible_item_count},
    },
    reovim_core::highlight::Style,
};

/// Mode for the prompt input (vim-style)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PromptMode {
    /// Insert mode - typing adds characters to query
    #[default]
    Insert,
    /// Normal mode - j/k navigation, vim motions in prompt
    Normal,
}

impl PromptMode {
    /// Get display string for mode
    #[must_use]
    pub const fn display(&self) -> &'static str {
        match self {
            Self::Insert => "[I]",
            Self::Normal => "[N]",
        }
    }
}

/// Loading state for async operations
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LoadingState {
    /// Not loading
    #[default]
    Idle,
    /// Loading items (show spinner)
    Loading,
    /// Matching in progress
    Matching,
}

impl LoadingState {
    /// Get spinner character for current state
    #[must_use]
    pub const fn spinner(&self) -> Option<char> {
        match self {
            Self::Idle => None,
            Self::Loading | Self::Matching => Some('⟳'),
        }
    }

    /// Check if currently loading
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        !matches!(self, Self::Idle)
    }
}

/// A styled span within a line (for syntax highlighting)
#[derive(Debug, Clone)]
pub struct StyledSpan {
    /// Start column (0-indexed, byte offset)
    pub start: usize,
    /// End column (exclusive, byte offset)
    pub end: usize,
    /// Style to apply
    pub style: Style,
}

impl StyledSpan {
    /// Create a new styled span
    #[must_use]
    pub const fn new(start: usize, end: usize, style: Style) -> Self {
        Self { start, end, style }
    }
}

/// Preview content for the selected item
#[derive(Debug, Clone, Default)]
pub struct PreviewContent {
    /// Lines of the preview content
    pub lines: Vec<String>,
    /// Line to highlight (0-indexed)
    pub highlight_line: Option<usize>,
    /// File extension for syntax highlighting
    pub syntax: Option<String>,
    /// Title for the preview panel
    pub title: Option<String>,
    /// Styled spans per line (for syntax highlighting)
    /// Each inner Vec contains spans for one line, sorted by start position
    pub styled_lines: Option<Vec<Vec<StyledSpan>>>,
}

impl PreviewContent {
    /// Create a new preview content
    #[must_use]
    pub const fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            highlight_line: None,
            syntax: None,
            title: None,
            styled_lines: None,
        }
    }

    /// Set styled lines for syntax highlighting
    #[must_use]
    pub fn with_styled_lines(mut self, styled_lines: Vec<Vec<StyledSpan>>) -> Self {
        self.styled_lines = Some(styled_lines);
        self
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

    /// Set the preview title
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// Legacy layout - kept for compatibility during transition
#[derive(Debug, Clone, Default)]
pub struct MicroscopeLayout {
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

/// State of the microscope fuzzy finder
#[derive(Debug, Clone, Default)]
pub struct MicroscopeState {
    /// Whether microscope is currently active/visible
    pub active: bool,
    /// Current search query
    pub query: String,
    /// Cursor position in the query
    pub cursor_pos: usize,
    /// All items from the picker (unfiltered)
    pub all_items: Vec<MicroscopeItem>,
    /// Current list of items (filtered/sorted)
    pub items: Vec<MicroscopeItem>,
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
    /// Legacy layout configuration (for compatibility)
    pub layout: MicroscopeLayout,
    /// Whether preview is enabled
    pub preview_enabled: bool,
    /// Prompt mode (Insert/Normal)
    pub prompt_mode: PromptMode,
    /// Loading state
    pub loading_state: LoadingState,
    /// Helix-style layout bounds
    pub bounds: LayoutBounds,
    /// Layout configuration
    pub layout_config: LayoutConfig,
    /// Total item count (from matcher)
    pub total_count: u32,
    /// Matched item count (from matcher)
    pub matched_count: u32,
}

impl MicroscopeState {
    /// Create a new empty microscope state
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
            layout: MicroscopeLayout::default(),
            preview_enabled: true,
            prompt_mode: PromptMode::Insert,
            loading_state: LoadingState::Idle,
            bounds: LayoutBounds::default(),
            layout_config: LayoutConfig::default(),
            total_count: 0,
            matched_count: 0,
        }
    }

    /// Open microscope with a picker
    ///
    /// Starts in Normal mode for j/k navigation. Press 'i' to enter Insert mode.
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
        self.prompt_mode = PromptMode::Normal; // Start in Normal mode
        self.loading_state = LoadingState::Loading;
        self.total_count = 0;
        self.matched_count = 0;
    }

    /// Close microscope
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
        self.prompt_mode = PromptMode::Insert;
        self.loading_state = LoadingState::Idle;
    }

    /// Enter insert mode
    pub fn enter_insert(&mut self) {
        self.prompt_mode = PromptMode::Insert;
    }

    /// Enter normal mode
    pub fn enter_normal(&mut self) {
        self.prompt_mode = PromptMode::Normal;
    }

    /// Toggle between insert and normal mode
    pub fn toggle_mode(&mut self) {
        self.prompt_mode = match self.prompt_mode {
            PromptMode::Insert => PromptMode::Normal,
            PromptMode::Normal => PromptMode::Insert,
        };
    }

    /// Set loading state
    pub fn set_loading(&mut self, state: LoadingState) {
        self.loading_state = state;
    }

    /// Update bounds from screen dimensions
    pub fn update_bounds(&mut self, screen_width: u16, screen_height: u16) {
        self.bounds = calculate_layout(screen_width, screen_height, &self.layout_config);
        // Also update legacy layout for compatibility
        self.layout.x = self.bounds.results.x;
        self.layout.y = self.bounds.results.y;
        self.layout.width = self.bounds.results.width;
        self.layout.height = self.bounds.results.height;
        self.layout.preview_width = self.bounds.preview.map(|p| p.width);
        self.layout.visible_items = visible_item_count(&self.bounds.results);
    }

    /// Get status line text
    #[must_use]
    pub fn status_text(&self) -> String {
        let count_text = if self.total_count == 0 {
            "No items".to_string()
        } else if self.matched_count == self.total_count {
            format!("{} items", self.total_count)
        } else {
            format!("{}/{} matched", self.matched_count, self.total_count)
        };

        let spinner = self
            .loading_state
            .spinner()
            .map_or(String::new(), |s| format!(" {s}"));

        format!("{count_text}{spinner}")
    }

    /// Update items from search results (initial load - stores in both `all_items` and items)
    pub fn update_items(&mut self, items: Vec<MicroscopeItem>) {
        self.all_items = items.clone();
        self.items = items;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.ensure_selected_visible();
    }

    /// Update filtered items only (for filtering - keeps `all_items` unchanged)
    pub fn update_filtered_items(&mut self, items: Vec<MicroscopeItem>) {
        self.items = items;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.ensure_selected_visible();
    }

    /// Insert a character at cursor position
    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.apply_filter();
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
            self.apply_filter();
        }
    }

    /// Apply query filter to items
    ///
    /// Simple substring filter. For fuzzy matching, use `MicroscopeMatcher`.
    fn apply_filter(&mut self) {
        if self.query.is_empty() {
            // Show all items when query is empty
            self.items = self.all_items.clone();
        } else {
            // Simple case-insensitive substring filter
            let query_lower = self.query.to_lowercase();
            self.items = self
                .all_items
                .iter()
                .filter(|item| item.match_text().to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
        }
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.matched_count = self.items.len() as u32;
        self.total_count = self.all_items.len() as u32;
        self.ensure_selected_visible();
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

    /// Move cursor forward one word
    pub fn word_forward(&mut self) {
        if self.cursor_pos >= self.query.len() {
            return;
        }

        let chars: Vec<char> = self.query.chars().collect();
        let mut pos = 0;
        let mut idx = 0;

        // Find current character index
        for (i, c) in self.query.char_indices() {
            if i >= self.cursor_pos {
                idx = pos;
                break;
            }
            pos += 1;
            if i + c.len_utf8() > self.cursor_pos {
                idx = pos;
                break;
            }
        }

        // Skip current word (non-whitespace)
        while idx < chars.len() && !chars[idx].is_whitespace() {
            idx += 1;
        }
        // Skip whitespace
        while idx < chars.len() && chars[idx].is_whitespace() {
            idx += 1;
        }

        // Convert back to byte position
        self.cursor_pos = chars[..idx].iter().map(|c| c.len_utf8()).sum();
    }

    /// Move cursor backward one word
    pub fn word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let chars: Vec<char> = self.query.chars().collect();

        // Find current character index
        let mut idx: usize = 0;
        let mut byte_pos = 0;
        for c in &chars {
            if byte_pos >= self.cursor_pos {
                break;
            }
            byte_pos += c.len_utf8();
            idx += 1;
        }

        idx = idx.saturating_sub(1);

        // Skip whitespace backward
        while idx > 0 && chars[idx].is_whitespace() {
            idx -= 1;
        }
        // Skip current word backward (non-whitespace)
        while idx > 0 && !chars[idx - 1].is_whitespace() {
            idx -= 1;
        }

        // Convert back to byte position
        self.cursor_pos = chars[..idx].iter().map(|c| c.len_utf8()).sum();
    }

    /// Clear the query
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.cursor_pos = 0;
        self.apply_filter();
    }

    /// Delete word before cursor
    pub fn delete_word(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let old_pos = self.cursor_pos;
        self.word_backward();
        let new_pos = self.cursor_pos;

        // Delete characters between new_pos and old_pos
        self.query.drain(new_pos..old_pos);
        self.apply_filter();
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
    pub fn selected_item(&self) -> Option<&MicroscopeItem> {
        self.items.get(self.selected_index)
    }

    /// Check if microscope is active and visible
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
    pub fn visible_items(&self) -> &[MicroscopeItem] {
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

            self.layout = MicroscopeLayout {
                x,
                y,
                width: results_width,
                height,
                preview_width: Some(preview_width),
                visible_items: usize::from(height.saturating_sub(4)), // -4 for borders and prompt
            };
        } else {
            self.layout = MicroscopeLayout {
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

#[cfg(test)]
mod tests {
    use {super::*, crate::microscope::item::MicroscopeData, std::path::PathBuf};

    fn sample_items() -> Vec<MicroscopeItem> {
        vec![
            MicroscopeItem::new(
                "1",
                "file1.rs",
                MicroscopeData::FilePath(PathBuf::from("file1.rs")),
                "files",
            ),
            MicroscopeItem::new(
                "2",
                "file2.rs",
                MicroscopeData::FilePath(PathBuf::from("file2.rs")),
                "files",
            ),
            MicroscopeItem::new(
                "3",
                "file3.rs",
                MicroscopeData::FilePath(PathBuf::from("file3.rs")),
                "files",
            ),
        ]
    }

    #[test]
    fn test_new_state() {
        let state = MicroscopeState::new();
        assert!(!state.active);
        assert!(state.query.is_empty());
        assert_eq!(state.cursor_pos, 0);
        assert!(state.items.is_empty());
    }

    #[test]
    fn test_open_close() {
        let mut state = MicroscopeState::new();
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
        let mut state = MicroscopeState::new();
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
        let mut state = MicroscopeState::new();
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
        let mut state = MicroscopeState::new();
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
