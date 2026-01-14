//! Theme override system for customizing individual colors/styles
//!
//! Allows users to override specific theme styles via TOML configuration
//! without modifying the base theme.
//!
//! # Example Configuration
//!
//! ```toml
//! [editor.theme_overrides]
//! "statusline.background" = { bg = "#1a1b26" }
//! "gutter.line_number" = { fg = "#565f89" }
//! "statusline.mode.normal" = { fg = "#1a1b26", bg = "#7aa2f7", bold = true }
//! ```

use {
    crate::highlight::{Attributes, Style, color::parse_color},
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

/// Error when applying a theme override
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeOverrideError {
    /// The path does not match any valid theme style
    InvalidPath(String),
}

impl std::fmt::Display for ThemeOverrideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(path) => write!(f, "Invalid theme override path: {path}"),
        }
    }
}

impl std::error::Error for ThemeOverrideError {}

/// Override for a single style field
///
/// All fields are optional - only specified fields will be applied.
/// Colors are specified as strings and parsed using `parse_color()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StyleOverride {
    /// Foreground color (e.g., "#ff0000", "rgb(255,0,0)", "red")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,

    /// Background color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,

    /// Underline color (only applies when underline is active)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<String>,

    // === Attributes ===
    /// Bold text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,

    /// Italic text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,

    /// Standard underline
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,

    /// Strikethrough text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,

    /// Dimmed text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<bool>,

    /// Reverse video (swap fg/bg)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<bool>,

    /// Blinking text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blink: Option<bool>,

    // === Extended underline styles ===
    /// Curly/wavy underline (for diagnostics)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curly_underline: Option<bool>,

    /// Dotted underline
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dotted_underline: Option<bool>,

    /// Dashed underline
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashed_underline: Option<bool>,

    /// Double underline
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_underline: Option<bool>,
}

impl StyleOverride {
    /// Apply this override to a style
    ///
    /// Only fields that are `Some` will be applied.
    /// Invalid color strings are logged as warnings and skipped.
    pub fn apply_to(&self, style: &mut Style) {
        // Colors
        if let Some(fg) = &self.fg {
            if let Some(color) = parse_color(fg) {
                style.fg = Some(color);
            } else {
                tracing::warn!(color = %fg, "Invalid foreground color in theme override");
            }
        }

        if let Some(bg) = &self.bg {
            if let Some(color) = parse_color(bg) {
                style.bg = Some(color);
            } else {
                tracing::warn!(color = %bg, "Invalid background color in theme override");
            }
        }

        if let Some(ul) = &self.underline_color {
            if let Some(color) = parse_color(ul) {
                style.underline_color = Some(color);
            } else {
                tracing::warn!(color = %ul, "Invalid underline color in theme override");
            }
        }

        // Standard attributes (only set if true, don't clear if false)
        if self.bold == Some(true) {
            style.attributes.set(Attributes::BOLD);
        }
        if self.italic == Some(true) {
            style.attributes.set(Attributes::ITALIC);
        }
        if self.underline == Some(true) {
            style.attributes.set(Attributes::UNDERLINE);
        }
        if self.strikethrough == Some(true) {
            style.attributes.set(Attributes::STRIKETHROUGH);
        }
        if self.dim == Some(true) {
            style.attributes.set(Attributes::DIM);
        }
        if self.reverse == Some(true) {
            style.attributes.set(Attributes::REVERSE);
        }
        if self.blink == Some(true) {
            style.attributes.set(Attributes::BLINK);
        }

        // Extended underline styles
        if self.curly_underline == Some(true) {
            style.attributes.set(Attributes::CURLY_UNDERLINE);
        }
        if self.dotted_underline == Some(true) {
            style.attributes.set(Attributes::DOTTED_UNDERLINE);
        }
        if self.dashed_underline == Some(true) {
            style.attributes.set(Attributes::DASHED_UNDERLINE);
        }
        if self.double_underline == Some(true) {
            style.attributes.set(Attributes::DOUBLE_UNDERLINE);
        }
    }
}

/// Theme overrides configuration
///
/// Maps style paths to their overrides.
/// Paths use dot notation: `"statusline.mode.normal"` or `"gutter.line_number"`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeOverrides {
    /// Map of style paths to overrides
    #[serde(flatten)]
    pub overrides: HashMap<String, StyleOverride>,
}

impl ThemeOverrides {
    /// Create empty overrides
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any overrides
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Get the number of overrides
    #[must_use]
    pub fn len(&self) -> usize {
        self.overrides.len()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_sys::style::Color};

    #[test]
    fn test_style_override_apply_fg_only() {
        let mut style = Style::new();
        let override_ = StyleOverride {
            fg: Some("#ff0000".to_string()),
            ..Default::default()
        };

        override_.apply_to(&mut style);

        assert_eq!(style.fg, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
        assert_eq!(style.bg, None);
    }

    #[test]
    fn test_style_override_apply_bg_only() {
        let mut style = Style::new();
        let override_ = StyleOverride {
            bg: Some("#1a1b26".to_string()),
            ..Default::default()
        };

        override_.apply_to(&mut style);

        assert_eq!(
            style.bg,
            Some(Color::Rgb {
                r: 26,
                g: 27,
                b: 38
            })
        );
        assert_eq!(style.fg, None);
    }

    #[test]
    fn test_style_override_apply_full() {
        let mut style = Style::new();
        let override_ = StyleOverride {
            fg: Some("white".to_string()),
            bg: Some("#7aa2f7".to_string()),
            bold: Some(true),
            italic: Some(true),
            ..Default::default()
        };

        override_.apply_to(&mut style);

        assert_eq!(style.fg, Some(Color::White));
        assert_eq!(
            style.bg,
            Some(Color::Rgb {
                r: 122,
                g: 162,
                b: 247
            })
        );
        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_style_override_preserves_unset() {
        let mut style = Style::new().fg(Color::Red).bg(Color::Blue).bold().italic();

        let override_ = StyleOverride {
            fg: Some("green".to_string()),
            ..Default::default()
        };

        override_.apply_to(&mut style);

        // fg changed
        assert_eq!(style.fg, Some(Color::Green));
        // bg preserved
        assert_eq!(style.bg, Some(Color::Blue));
        // attributes preserved
        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_style_override_invalid_color_skipped() {
        let mut style = Style::new().fg(Color::Red);
        let override_ = StyleOverride {
            fg: Some("invalid_color".to_string()),
            ..Default::default()
        };

        override_.apply_to(&mut style);

        // Original color preserved (invalid was skipped)
        assert_eq!(style.fg, Some(Color::Red));
    }

    #[test]
    fn test_theme_overrides_serde() {
        let toml = r##"
"statusline.background" = { bg = "#1a1b26" }
"gutter.line_number" = { fg = "#565f89" }
"##;

        let overrides: ThemeOverrides = toml::from_str(toml).unwrap();
        assert_eq!(overrides.len(), 2);
        assert!(overrides.overrides.contains_key("statusline.background"));
        assert!(overrides.overrides.contains_key("gutter.line_number"));
    }

    #[test]
    fn test_theme_overrides_serde_with_attributes() {
        let toml = r##"
"statusline.mode.normal" = { fg = "#1a1b26", bg = "#7aa2f7", bold = true }
"##;

        let overrides: ThemeOverrides = toml::from_str(toml).unwrap();
        let normal_override = overrides.overrides.get("statusline.mode.normal").unwrap();

        assert_eq!(normal_override.fg, Some("#1a1b26".to_string()));
        assert_eq!(normal_override.bg, Some("#7aa2f7".to_string()));
        assert_eq!(normal_override.bold, Some(true));
    }
}
