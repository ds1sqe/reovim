//! TOML theme file parsing and loading.
//!
//! This module provides types for parsing user-defined themes from
//! TOML files. Themes follow a Helix/Zed-inspired format with
//! palette, syntax, UI, diagnostic, and gutter sections.
//!
//! `FileTheme` implements both [`ThemeProvider`] (the registry's slim
//! trait) and [`StyledTheme`] (the display-tier `Style`-aware
//! super-trait). The registered [`DisplayThemeFactory`] dispatches
//! parsing here from the registry crate's `ThemeLoader`.
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
//!
//! [`DisplayThemeFactory`]: super::factory::DisplayThemeFactory
//! [`ThemeProvider`]: reovim_driver_display_registry::theme::ThemeProvider
//! [`StyledTheme`]: super::theme::StyledTheme

use std::{any::Any, collections::HashMap, sync::Arc};

use {
    reovim_arch::Color,
    reovim_driver_display_registry::theme::{ThemeError, ThemeProvider},
    serde::Deserialize,
};

use crate::highlight::{Attributes, Style};

use super::theme::StyledTheme;

/// Wrap a `toml::de::Error` into the registry-tier [`ThemeError::Parse`].
fn parse_error(err: toml::de::Error) -> ThemeError {
    ThemeError::Parse(Box::new(err))
}

// =============================================================================
// TOML Schema Types
// =============================================================================

/// Raw theme file structure as parsed from TOML.
///
/// Public so internal tests and any future direct parsing tooling can
/// reach it; consumers usually go through [`FileTheme::parse`].
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

    /// Decoration styles (heading decorations, virtual text, etc.).
    #[serde(default)]
    pub decoration: HashMap<String, StyleDef>,

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

    /// Base built-in theme to inherit from.
    #[serde(default)]
    pub base: Option<String>,
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
#[allow(clippy::struct_excessive_bools)]
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
/// This is the runtime representation of a user theme. All palette
/// references have been resolved to actual colors.
#[derive(Debug)]
pub struct FileTheme {
    /// Theme name.
    name: String,

    /// Author (optional).
    #[allow(dead_code)]
    author: Option<String>,

    /// Resolved styles indexed by group name.
    styles: HashMap<String, Style>,

    /// Default foreground style.
    default_style: Style,
}

impl FileTheme {
    /// Parse a theme from TOML string content.
    ///
    /// # Errors
    ///
    /// Returns [`ThemeError`] if:
    /// - TOML syntax is invalid (wrapped as
    ///   [`ThemeError::Parse`]),
    /// - a color value cannot be parsed,
    /// - a palette reference is undefined,
    /// - a `meta.base` reference does not match a built-in theme.
    pub fn parse(content: &str) -> Result<Self, ThemeError> {
        let file: ThemeFile = toml::from_str(content).map_err(parse_error)?;
        Self::from_file(file)
    }

    /// Resolve a base theme name to a [`super::BuiltinTheme`] variant.
    fn resolve_base_theme(name: &str) -> Option<super::BuiltinTheme> {
        super::BuiltinTheme::all()
            .iter()
            .find(|v| v.name() == name)
            .copied()
    }

    /// Convert a parsed `ThemeFile` into a resolved `FileTheme`.
    fn from_file(file: ThemeFile) -> Result<Self, ThemeError> {
        let mut styles = HashMap::new();

        if let Some(ref base_name) = file.meta.base {
            let base_variant =
                Self::resolve_base_theme(base_name).ok_or_else(|| ThemeError::InvalidBase {
                    name: base_name.clone(),
                })?;
            let base_palette = super::builtin::get_palette(base_variant);
            for (key, style) in base_palette {
                styles.insert((*key).to_string(), style.clone());
            }
        }

        Self::resolve_section(&file.palette, &file.syntax, &mut styles, "")?;
        Self::resolve_section(&file.palette, &file.ui, &mut styles, "")?;
        Self::resolve_section(&file.palette, &file.diagnostic, &mut styles, "diagnostic.")?;
        Self::resolve_section(&file.palette, &file.decoration, &mut styles, "decoration.")?;
        Self::resolve_section(&file.palette, &file.gutter, &mut styles, "")?;

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

    /// Resolve a single style definition to a `Style`.
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

    /// Resolve a color string to a `Color`.
    ///
    /// Lookup order:
    /// 1. Palette reference (if key exists in palette).
    /// 2. Hex color (`#rrggbb` or `#rgb`).
    /// 3. Named ANSI color (red, blue, etc.).
    fn resolve_color(
        palette: &HashMap<String, String>,
        key: &str,
        value: &str,
    ) -> Result<Color, ThemeError> {
        if let Some(resolved) = palette.get(value) {
            return Self::parse_color_value(key, resolved);
        }

        Self::parse_color_value(key, value)
    }

    /// Parse a color value string to `Color`.
    fn parse_color_value(key: &str, value: &str) -> Result<Color, ThemeError> {
        Color::parse(value).ok_or_else(|| ThemeError::InvalidColor {
            key: key.to_string(),
            value: value.to_string(),
        })
    }

    /// Load this theme as an `Arc<dyn ThemeProvider>` for use with
    /// the slim
    /// [`ThemeManager`](reovim_driver_display_registry::theme::ThemeManager).
    #[must_use]
    pub fn into_arc(self) -> Arc<dyn ThemeProvider> {
        Arc::new(self)
    }
}

impl ThemeProvider for FileTheme {
    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl StyledTheme for FileTheme {
    fn get_style(&self, group: &str) -> Option<Style> {
        self.styles.get(group).cloned()
    }

    fn default_style(&self) -> Style {
        self.default_style.clone()
    }
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
