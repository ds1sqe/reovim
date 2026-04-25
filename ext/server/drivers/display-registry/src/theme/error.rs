//! Theme-loading errors.
//!
//! `ThemeError::Parse` carries a boxed `dyn Error + Send + Sync` so the
//! registry crate does not depend on `toml`. The display-tier parser
//! (`reovim-driver-display::style::file::FileTheme`) wraps its own
//! `toml::de::Error` into this variant when implementing
//! [`super::factory::ThemeFactory::load_file`].

use std::{error::Error, fmt, io};

/// Errors that can occur when loading a theme file.
#[derive(Debug)]
pub enum ThemeError {
    /// Failed to read the theme file.
    Io(io::Error),
    /// Failed to parse the theme content.
    ///
    /// The boxed inner error is the parser's native error type (e.g.
    /// `toml::de::Error` on the display side); registry consumers only
    /// need `Display` + `Error`, so the box is sufficient.
    Parse(Box<dyn Error + Send + Sync>),
    /// Invalid color value in theme.
    InvalidColor {
        /// Style key whose color failed to parse (e.g. `"keyword"`).
        key: String,
        /// Raw color value as it appeared in the file.
        value: String,
    },
    /// Referenced palette color not found.
    PaletteNotFound {
        /// Style key whose palette reference failed.
        key: String,
        /// Palette name that could not be resolved.
        reference: String,
    },
    /// Invalid base theme name.
    InvalidBase {
        /// Base theme name that does not match any built-in variant.
        name: String,
    },
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Parse(e) => write!(f, "TOML parse error: {e}"),
            Self::InvalidColor { key, value } => {
                write!(f, "Invalid color '{value}' for key '{key}'")
            }
            Self::PaletteNotFound { key, reference } => {
                write!(f, "Palette color '{reference}' not found for key '{key}'")
            }
            Self::InvalidBase { name } => {
                write!(f, "Invalid base theme: '{name}'")
            }
        }
    }
}

impl Error for ThemeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(&**e),
            Self::InvalidColor { .. } | Self::PaletteNotFound { .. } | Self::InvalidBase { .. } => {
                None
            }
        }
    }
}

impl From<io::Error> for ThemeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
