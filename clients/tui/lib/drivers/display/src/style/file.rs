//! TOML theme file parsing and loading.
//!
//! This module provides types for parsing user-defined themes from TOML files.
//! Themes follow a Helix/Zed-inspired format with palette, syntax, UI,
//! diagnostic, and gutter sections.
//!
//! # File Format
//!
//! ```toml
//! [meta]
//! name = "My Theme"
//! author = "Author Name"
//! version = "1.0"
//!
//! [palette]
//! red = "#ff0000"
//! blue = "#0000ff"
//!
//! [syntax]
//! keyword = { fg = "red", bold = true }
//! function = { fg = "blue" }
//!
//! [ui]
//! background = { bg = "#1a1b26" }
//!
//! [diagnostic]
//! error = { fg = "red" }
//!
//! [gutter]
//! sign.add = { fg = "#98c379" }
//! ```

use std::{collections::HashMap, sync::Arc};

use {reovim_arch::Color, serde::Deserialize};

use crate::highlight::{Attributes, Style};

use super::ThemeProvider;

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur when loading a theme file.
#[derive(Debug)]
pub enum ThemeError {
    /// Failed to read the theme file.
    Io(std::io::Error),
    /// Failed to parse TOML syntax.
    Parse(toml::de::Error),
    /// Invalid color value in theme.
    InvalidColor { key: String, value: String },
    /// Referenced palette color not found.
    PaletteNotFound { key: String, reference: String },
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Parse(e) => write!(f, "TOML parse error: {e}"),
            Self::InvalidColor { key, value } => {
                write!(f, "Invalid color '{value}' for key '{key}'")
            }
            Self::PaletteNotFound { key, reference } => {
                write!(f, "Palette color '{reference}' not found for key '{key}'")
            }
        }
    }
}

impl std::error::Error for ThemeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::InvalidColor { .. } | Self::PaletteNotFound { .. } => None,
        }
    }
}

impl From<std::io::Error> for ThemeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for ThemeError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

// =============================================================================
// TOML Schema Types
// =============================================================================

/// Raw theme file structure as parsed from TOML.
#[derive(Debug, Deserialize)]
pub struct ThemeFile {
    /// Theme metadata.
    #[serde(default)]
    pub meta: ThemeMeta,

    /// Named color palette for reuse in style definitions.
    #[serde(default)]
    pub palette: HashMap<String, String>,

    /// Syntax highlighting styles (keyword, function, type, etc.).
    #[serde(default)]
    pub syntax: HashMap<String, StyleDef>,

    /// UI element styles (background, cursor, statusline, etc.).
    #[serde(default)]
    pub ui: HashMap<String, StyleDef>,

    /// Diagnostic styles (error, warning, info, hint).
    #[serde(default)]
    pub diagnostic: HashMap<String, StyleDef>,

    /// Gutter/annotation styles (git signs, fold markers, etc.).
    #[serde(default)]
    pub gutter: HashMap<String, StyleDef>,
}

/// Theme metadata.
#[derive(Debug, Default, Deserialize)]
pub struct ThemeMeta {
    /// Theme display name.
    #[serde(default = "default_theme_name")]
    pub name: String,

    /// Theme author.
    #[serde(default)]
    pub author: Option<String>,

    /// Theme version.
    #[serde(default)]
    pub version: Option<String>,
}

fn default_theme_name() -> String {
    "Unnamed Theme".to_string()
}

/// Style definition as specified in TOML.
///
/// Colors can be:
/// - Hex color: `"#ff0000"`
/// - Named color: `"red"`, `"blue"`
/// - Palette reference: `"my_red"` (if defined in `[palette]`)
#[derive(Debug, Default, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Bools are natural for style attributes in TOML
pub struct StyleDef {
    /// Foreground color.
    pub fg: Option<String>,

    /// Background color.
    pub bg: Option<String>,

    /// Bold attribute.
    #[serde(default)]
    pub bold: bool,

    /// Italic attribute.
    #[serde(default)]
    pub italic: bool,

    /// Underline attribute.
    #[serde(default)]
    pub underline: bool,

    /// Strikethrough attribute.
    #[serde(default)]
    pub strikethrough: bool,

    /// Underline color (for colored underlines).
    pub underline_color: Option<String>,
}

// =============================================================================
// FileTheme - Resolved Theme
// =============================================================================

/// A theme loaded from a TOML file with resolved styles.
///
/// This is the runtime representation of a user theme. All palette references
/// have been resolved to actual colors.
#[derive(Debug)]
pub struct FileTheme {
    /// Theme name.
    name: String,

    /// Author (optional).
    #[allow(dead_code)]
    author: Option<String>,

    /// Resolved styles indexed by group name.
    /// Keys are normalized: `keyword`, `keyword.control`, `diagnostic.error`, etc.
    styles: HashMap<String, Style>,

    /// Default foreground style.
    default_style: Style,
}

impl FileTheme {
    /// Parse a theme from TOML string content.
    ///
    /// # Errors
    ///
    /// Returns `ThemeError` if:
    /// - TOML syntax is invalid
    /// - A color value cannot be parsed
    /// - A palette reference is undefined
    pub fn parse(content: &str) -> Result<Self, ThemeError> {
        let file: ThemeFile = toml::from_str(content)?;
        Self::from_file(file)
    }

    /// Convert a parsed `ThemeFile` into a resolved `FileTheme`.
    fn from_file(file: ThemeFile) -> Result<Self, ThemeError> {
        let mut styles = HashMap::new();

        // Resolve all style sections
        Self::resolve_section(&file.palette, &file.syntax, &mut styles, "")?;
        Self::resolve_section(&file.palette, &file.ui, &mut styles, "")?;
        Self::resolve_section(&file.palette, &file.diagnostic, &mut styles, "diagnostic.")?;
        Self::resolve_section(&file.palette, &file.gutter, &mut styles, "")?;

        // Determine default style from foreground or fallback
        let default_style = styles
            .get("foreground")
            .cloned()
            .unwrap_or_else(Style::default);

        Ok(Self {
            name: file.meta.name,
            author: file.meta.author,
            styles,
            default_style,
        })
    }

    /// Resolve a section of style definitions into the styles map.
    fn resolve_section(
        palette: &HashMap<String, String>,
        section: &HashMap<String, StyleDef>,
        styles: &mut HashMap<String, Style>,
        prefix: &str,
    ) -> Result<(), ThemeError> {
        for (key, def) in section {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}{key}")
            };

            let style = Self::resolve_style_def(palette, &full_key, def)?;
            styles.insert(full_key, style);
        }
        Ok(())
    }

    /// Resolve a single style definition to a Style.
    fn resolve_style_def(
        palette: &HashMap<String, String>,
        key: &str,
        def: &StyleDef,
    ) -> Result<Style, ThemeError> {
        let fg = def
            .fg
            .as_ref()
            .map(|c| Self::resolve_color(palette, key, c))
            .transpose()?;

        let bg = def
            .bg
            .as_ref()
            .map(|c| Self::resolve_color(palette, key, c))
            .transpose()?;

        let underline_color = def
            .underline_color
            .as_ref()
            .map(|c| Self::resolve_color(palette, key, c))
            .transpose()?;

        let mut attributes = Attributes::new();
        if def.bold {
            attributes.set(Attributes::BOLD);
        }
        if def.italic {
            attributes.set(Attributes::ITALIC);
        }
        if def.underline {
            attributes.set(Attributes::UNDERLINE);
        }
        if def.strikethrough {
            attributes.set(Attributes::STRIKETHROUGH);
        }

        Ok(Style {
            fg,
            bg,
            attributes,
            underline_color,
        })
    }

    /// Resolve a color string to a Color.
    ///
    /// Lookup order:
    /// 1. Palette reference (if key exists in palette)
    /// 2. Hex color (#rrggbb or #rgb)
    /// 3. Named ANSI color (red, blue, etc.)
    fn resolve_color(
        palette: &HashMap<String, String>,
        key: &str,
        value: &str,
    ) -> Result<Color, ThemeError> {
        // Check palette first
        if let Some(resolved) = palette.get(value) {
            return Self::parse_color_value(key, resolved);
        }

        // Parse directly
        Self::parse_color_value(key, value)
    }

    /// Parse a color value string to Color.
    fn parse_color_value(key: &str, value: &str) -> Result<Color, ThemeError> {
        // Try Color::parse which handles hex and named colors
        Color::parse(value).ok_or_else(|| ThemeError::InvalidColor {
            key: key.to_string(),
            value: value.to_string(),
        })
    }

    /// Load this theme as an Arc for use with `ThemeManager`.
    #[must_use]
    pub fn into_arc(self) -> Arc<dyn ThemeProvider> {
        Arc::new(self)
    }
}

impl ThemeProvider for FileTheme {
    fn get_style(&self, group: &str) -> Option<Style> {
        self.styles.get(group).cloned()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn default_style(&self) -> Style {
        self.default_style.clone()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_theme() {
        let toml = r#"
            [meta]
            name = "Test Theme"
        "#;

        let theme = FileTheme::parse(toml).unwrap();
        assert_eq!(theme.name(), "Test Theme");
    }

    #[test]
    fn test_parse_theme_with_palette() {
        let toml = r##"
            [meta]
            name = "Palette Test"

            [palette]
            red = "#ff0000"
            blue = "#0000ff"

            [syntax]
            keyword = { fg = "red", bold = true }
            function = { fg = "blue" }
        "##;

        let theme = FileTheme::parse(toml).unwrap();
        let keyword = theme.get_style("keyword").unwrap();
        assert_eq!(keyword.fg, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
        assert!(keyword.attributes.contains(Attributes::BOLD));

        let function = theme.get_style("function").unwrap();
        assert_eq!(function.fg, Some(Color::Rgb { r: 0, g: 0, b: 255 }));
    }

    #[test]
    fn test_parse_hex_colors_directly() {
        let toml = r##"
            [meta]
            name = "Direct Hex"

            [ui]
            background = { bg = "#1a1b26" }
            cursor = { fg = "#ffffff", bg = "#ff9e64" }
        "##;

        let theme = FileTheme::parse(toml).unwrap();
        let bg = theme.get_style("background").unwrap();
        assert_eq!(
            bg.bg,
            Some(Color::Rgb {
                r: 26,
                g: 27,
                b: 38
            })
        );

        let cursor = theme.get_style("cursor").unwrap();
        assert_eq!(
            cursor.fg,
            Some(Color::Rgb {
                r: 255,
                g: 255,
                b: 255
            })
        );
        assert_eq!(
            cursor.bg,
            Some(Color::Rgb {
                r: 255,
                g: 158,
                b: 100
            })
        );
    }

    #[test]
    fn test_parse_diagnostic_section() {
        let toml = r##"
            [meta]
            name = "Diagnostic Test"

            [diagnostic]
            error = { fg = "#f7768e" }
            warn = { fg = "#e0af68" }
        "##;

        let theme = FileTheme::parse(toml).unwrap();

        // Diagnostic keys get "diagnostic." prefix
        let error = theme.get_style("diagnostic.error").unwrap();
        assert_eq!(
            error.fg,
            Some(Color::Rgb {
                r: 247,
                g: 118,
                b: 142
            })
        );
    }

    #[test]
    fn test_parse_gutter_section() {
        let toml = r##"
            [meta]
            name = "Gutter Test"

            [gutter]
            "git.add" = { fg = "#98c379" }
            "fold.open" = { fg = "#565f89" }
        "##;

        let theme = FileTheme::parse(toml).unwrap();
        let git_add = theme.get_style("git.add").unwrap();
        assert_eq!(
            git_add.fg,
            Some(Color::Rgb {
                r: 152,
                g: 195,
                b: 121
            })
        );
    }

    #[test]
    fn test_parse_all_attributes() {
        let toml = r##"
            [meta]
            name = "Attributes Test"

            [syntax]
            test = { fg = "#ffffff", bold = true, italic = true, underline = true, strikethrough = true }
        "##;

        let theme = FileTheme::parse(toml).unwrap();
        let style = theme.get_style("test").unwrap();
        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::ITALIC));
        assert!(style.attributes.contains(Attributes::UNDERLINE));
        assert!(style.attributes.contains(Attributes::STRIKETHROUGH));
    }

    #[test]
    fn test_invalid_color_error() {
        let toml = r#"
            [meta]
            name = "Invalid Color"

            [syntax]
            keyword = { fg = "not-a-color" }
        "#;

        let result = FileTheme::parse(toml);
        assert!(result.is_err());
        match result.unwrap_err() {
            ThemeError::InvalidColor { key, value } => {
                assert_eq!(key, "keyword");
                assert_eq!(value, "not-a-color");
            }
            _ => panic!("Expected InvalidColor error"),
        }
    }

    #[test]
    fn test_named_ansi_colors() {
        let toml = r#"
            [meta]
            name = "ANSI Test"

            [syntax]
            keyword = { fg = "red" }
            string = { fg = "green" }
        "#;

        let theme = FileTheme::parse(toml).unwrap();
        let keyword = theme.get_style("keyword").unwrap();
        assert_eq!(keyword.fg, Some(Color::Red));

        let string = theme.get_style("string").unwrap();
        assert_eq!(string.fg, Some(Color::Green));
    }

    #[test]
    fn test_default_style_from_foreground() {
        let toml = r##"
            [meta]
            name = "Default Style Test"

            [ui]
            foreground = { fg = "#abcdef" }
        "##;

        let theme = FileTheme::parse(toml).unwrap();
        let default = theme.default_style();
        assert_eq!(
            default.fg,
            Some(Color::Rgb {
                r: 171,
                g: 205,
                b: 239
            })
        );
    }

    #[test]
    fn test_into_arc() {
        let toml = r#"
            [meta]
            name = "Arc Test"
        "#;

        let theme = FileTheme::parse(toml).unwrap();
        let arc_theme = theme.into_arc();
        assert_eq!(arc_theme.name(), "Arc Test");
    }
}
