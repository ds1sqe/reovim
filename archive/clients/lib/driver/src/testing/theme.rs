//! Mock theme provider for testing.

use std::collections::HashMap;

use crate::{Style, ThemeProvider, types::Color};

/// Configurable mock for `ThemeProvider`.
///
/// Returns `Style::default()` for all groups unless specific highlights are seeded.
pub struct MockThemeProvider {
    highlights: HashMap<String, Style>,
}

impl MockThemeProvider {
    /// Create with no seeded highlights (returns `Style::default()` for everything).
    #[must_use]
    pub fn new() -> Self {
        Self {
            highlights: HashMap::new(),
        }
    }

    /// Seed a highlight group style (builder pattern).
    #[must_use]
    pub fn with_highlight(mut self, group: &str, style: Style) -> Self {
        self.highlights.insert(group.to_string(), style);
        self
    }
}

impl Default for MockThemeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeProvider for MockThemeProvider {
    fn highlight(&self, group: &str) -> Style {
        self.highlights.get(group).cloned().unwrap_or_default()
    }

    fn highlight_with_fallback(&self, groups: &[&str]) -> Style {
        for group in groups {
            if let Some(style) = self.highlights.get(*group) {
                return style.clone();
            }
        }
        Style::default()
    }

    fn foreground(&self) -> Style {
        Style {
            fg: Some(Color::White),
            ..Style::default()
        }
    }

    fn background(&self) -> Style {
        Style {
            bg: Some(Color::Black),
            ..Style::default()
        }
    }

    fn is_dark(&self) -> bool {
        true
    }
}
