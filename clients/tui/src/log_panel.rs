//! Log panel state for the TUI.
//!
//! Tracks visibility, scroll position, and level filtering for the log panel.

use reovim_protocol::v1::LogLevel;

/// Default panel height in rows.
pub const DEFAULT_PANEL_HEIGHT: u16 = 10;

/// Log panel state.
#[derive(Debug)]
pub struct LogPanelState {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Scroll offset from the bottom (0 = newest entries visible).
    pub scroll_offset: usize,
    /// Panel height in rows.
    pub height: u16,
    /// Level filter (None = show all).
    pub level_filter: Option<LogLevel>,
    /// Auto-scroll to newest entries when new logs arrive.
    pub auto_scroll: bool,
}

impl LogPanelState {
    /// Create a new log panel state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            visible: false,
            scroll_offset: 0,
            height: DEFAULT_PANEL_HEIGHT,
            level_filter: None,
            auto_scroll: true,
        }
    }

    /// Toggle panel visibility.
    ///
    /// Returns the new visibility state.
    pub const fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        if self.visible {
            // Reset scroll to bottom when showing
            self.scroll_offset = 0;
            self.auto_scroll = true;
        }
        self.visible
    }

    /// Show the panel.
    pub const fn show(&mut self) {
        if !self.visible {
            self.visible = true;
            self.scroll_offset = 0;
            self.auto_scroll = true;
        }
    }

    /// Hide the panel.
    pub const fn hide(&mut self) {
        self.visible = false;
    }

    /// Scroll up (toward older entries).
    pub const fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
        self.auto_scroll = false;
    }

    /// Scroll down (toward newer entries).
    pub const fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
        if self.scroll_offset == 0 {
            self.auto_scroll = true;
        }
    }

    /// Scroll to the bottom (newest entries).
    pub const fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Scroll to the top (oldest entries).
    pub const fn scroll_to_top(&mut self, total_entries: usize) {
        let visible = self.height as usize;
        self.scroll_offset = total_entries.saturating_sub(visible);
        self.auto_scroll = false;
    }

    /// Set the level filter.
    pub const fn set_level_filter(&mut self, level: Option<LogLevel>) {
        self.level_filter = level;
        // Reset scroll when changing filter
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Set the level filter from a number key (1-5).
    ///
    /// - 1 = Error only
    /// - 2 = Warn and above
    /// - 3 = Info and above
    /// - 4 = Debug and above
    /// - 5 = Trace (all)
    pub const fn set_level_filter_from_key(&mut self, key: char) {
        let level = match key {
            '1' => Some(LogLevel::Error),
            '2' => Some(LogLevel::Warn),
            '3' => Some(LogLevel::Info),
            '4' => Some(LogLevel::Debug),
            '5' => None, // All levels (including trace)
            _ => return,
        };
        self.set_level_filter(level);
    }

    /// Check if an entry passes the level filter.
    #[must_use]
    pub fn passes_filter(&self, level: LogLevel) -> bool {
        self.level_filter.is_none_or(|min_level| level >= min_level)
    }

    /// Set panel height.
    pub fn set_height(&mut self, height: u16) {
        self.height = height.max(3); // Minimum 3 rows (header + 1 entry + status)
    }

    /// Called when new entries are added.
    ///
    /// If `auto_scroll` is enabled, keeps scroll at bottom.
    pub const fn on_new_entries(&mut self) {
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }
}

impl Default for LogPanelState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "log_panel_tests.rs"]
mod tests;
