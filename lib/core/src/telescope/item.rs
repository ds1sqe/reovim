//! Telescope item types

use std::path::PathBuf;

use crate::{command::CommandId, highlight::ThemeName};

/// Data associated with a telescope item, determining the action on selection
#[derive(Debug, Clone)]
pub enum TelescopeData {
    /// A file path to open
    FilePath(PathBuf),
    /// A buffer ID to switch to
    BufferId(usize),
    /// A command to execute
    Command(CommandId),
    /// A help tag to display
    HelpTag(String),
    /// A keymap entry
    Keymap {
        mode: String,
        key: String,
        command: String,
    },
    /// A grep match with location
    GrepMatch {
        path: PathBuf,
        line: usize,
        col: usize,
    },
    /// A theme/colorscheme to apply
    Theme(ThemeName),
    /// A configuration profile to load
    Profile(String),
}

/// Represents a single item in the telescope results list
#[derive(Debug, Clone)]
pub struct TelescopeItem {
    /// Unique identifier for this item
    pub id: String,
    /// Primary display text
    pub display: String,
    /// Secondary detail text (optional, shown grayed out)
    pub detail: Option<String>,
    /// Data for determining action on selection
    pub data: TelescopeData,
    /// Fuzzy match score (higher = better match)
    pub score: Option<u32>,
    /// Icon/indicator character
    pub icon: Option<char>,
    /// Source picker name
    pub source: &'static str,
}

impl TelescopeItem {
    /// Create a new telescope item
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        display: impl Into<String>,
        data: TelescopeData,
        source: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
            detail: None,
            data,
            score: None,
            icon: None,
            source,
        }
    }

    /// Set detail text
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set match score
    #[must_use]
    pub const fn with_score(mut self, score: u32) -> Self {
        self.score = Some(score);
        self
    }

    /// Set icon
    #[must_use]
    pub const fn with_icon(mut self, icon: char) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Get the text used for matching
    #[must_use]
    pub fn match_text(&self) -> &str {
        &self.display
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_item() {
        let item = TelescopeItem::new(
            "file1",
            "src/main.rs",
            TelescopeData::FilePath(PathBuf::from("src/main.rs")),
            "files",
        );

        assert_eq!(item.id, "file1");
        assert_eq!(item.display, "src/main.rs");
        assert_eq!(item.source, "files");
        assert!(item.detail.is_none());
        assert!(item.score.is_none());
        assert!(item.icon.is_none());
    }

    #[test]
    fn test_builder_methods() {
        let item = TelescopeItem::new("buf1", "buffer.rs", TelescopeData::BufferId(1), "buffers")
            .with_detail("modified")
            .with_score(100)
            .with_icon('*');

        assert_eq!(item.detail.as_deref(), Some("modified"));
        assert_eq!(item.score, Some(100));
        assert_eq!(item.icon, Some('*'));
    }
}
