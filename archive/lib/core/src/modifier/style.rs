//! Style modifiers for visual appearance changes

use {crate::screen::BorderStyle, reovim_sys::style::Color};

/// Window decoration options
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowDecorations {
    /// Show line numbers
    pub line_numbers: Option<bool>,
    /// Show relative line numbers
    pub relative_numbers: Option<bool>,
    /// Show scrollbar
    pub scrollbar: Option<bool>,
    /// Show indent guides
    pub indent_guides: Option<bool>,
}

impl WindowDecorations {
    /// Create empty decorations (no overrides)
    #[must_use]
    pub const fn none() -> Self {
        Self {
            line_numbers: None,
            relative_numbers: None,
            scrollbar: None,
            indent_guides: None,
        }
    }

    /// Merge with another decorations, preferring other's values when set
    #[must_use]
    pub const fn merge(&self, other: &Self) -> Self {
        Self {
            line_numbers: if other.line_numbers.is_some() {
                other.line_numbers
            } else {
                self.line_numbers
            },
            relative_numbers: if other.relative_numbers.is_some() {
                other.relative_numbers
            } else {
                self.relative_numbers
            },
            scrollbar: if other.scrollbar.is_some() {
                other.scrollbar
            } else {
                self.scrollbar
            },
            indent_guides: if other.indent_guides.is_some() {
                other.indent_guides
            } else {
                self.indent_guides
            },
        }
    }
}

/// Style modifiers that can be applied to windows
#[derive(Debug, Clone, Default)]
pub struct StyleModifiers {
    /// Override border color
    pub border_color: Option<Color>,
    /// Override background color
    pub background_color: Option<Color>,
    /// Override foreground color
    pub foreground_color: Option<Color>,
    /// Override border style
    pub border_style: Option<BorderStyle>,
    /// Window decorations overrides
    pub decorations: WindowDecorations,
}

impl StyleModifiers {
    /// Create empty style modifiers
    #[must_use]
    pub const fn none() -> Self {
        Self {
            border_color: None,
            background_color: None,
            foreground_color: None,
            border_style: None,
            decorations: WindowDecorations::none(),
        }
    }

    /// Check if any style is set
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.border_color.is_none()
            && self.background_color.is_none()
            && self.foreground_color.is_none()
            && self.border_style.is_none()
            && self.decorations.line_numbers.is_none()
            && self.decorations.relative_numbers.is_none()
            && self.decorations.scrollbar.is_none()
            && self.decorations.indent_guides.is_none()
    }

    /// Merge with another style modifiers, preferring other's values when set
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            border_color: other.border_color.or(self.border_color),
            background_color: other.background_color.or(self.background_color),
            foreground_color: other.foreground_color.or(self.foreground_color),
            border_style: other.border_style.or(self.border_style),
            decorations: self.decorations.merge(&other.decorations),
        }
    }

    /// Create with border color
    #[must_use]
    pub const fn with_border_color(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Create with background color
    #[must_use]
    pub const fn with_background(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Create with border style
    #[must_use]
    pub const fn with_border_style(mut self, style: BorderStyle) -> Self {
        self.border_style = Some(style);
        self
    }
}

/// Computed style state for a window after applying all modifiers
#[derive(Debug, Clone, Default)]
pub struct WindowStyleState {
    /// Effective style modifiers
    pub style: StyleModifiers,
    /// IDs of modifiers that contributed to this state
    pub applied_modifiers: Vec<&'static str>,
}

impl WindowStyleState {
    /// Create empty state
    #[must_use]
    pub const fn new() -> Self {
        Self {
            style: StyleModifiers::none(),
            applied_modifiers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_modifiers_empty() {
        let style = StyleModifiers::none();
        assert!(style.is_empty());
    }

    #[test]
    fn test_style_modifiers_merge() {
        let base = StyleModifiers::none().with_border_color(Color::Rgb { r: 255, g: 0, b: 0 });
        let overlay = StyleModifiers::none().with_background(Color::Rgb { r: 0, g: 0, b: 255 });

        let merged = base.merge(&overlay);

        assert_eq!(merged.border_color, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
        assert_eq!(merged.background_color, Some(Color::Rgb { r: 0, g: 0, b: 255 }));
    }

    #[test]
    fn test_style_modifiers_override() {
        let base = StyleModifiers::none().with_border_color(Color::Rgb { r: 255, g: 0, b: 0 });
        let overlay = StyleModifiers::none().with_border_color(Color::Rgb { r: 0, g: 255, b: 0 });

        let merged = base.merge(&overlay);

        assert_eq!(merged.border_color, Some(Color::Rgb { r: 0, g: 255, b: 0 }));
    }

    #[test]
    fn test_decorations_merge() {
        let base = WindowDecorations {
            line_numbers: Some(true),
            relative_numbers: None,
            scrollbar: Some(false),
            indent_guides: None,
        };

        let overlay = WindowDecorations {
            line_numbers: None,
            relative_numbers: Some(true),
            scrollbar: None,
            indent_guides: Some(true),
        };

        let merged = base.merge(&overlay);

        assert_eq!(merged.line_numbers, Some(true)); // From base
        assert_eq!(merged.relative_numbers, Some(true)); // From overlay
        assert_eq!(merged.scrollbar, Some(false)); // From base
        assert_eq!(merged.indent_guides, Some(true)); // From overlay
    }
}
