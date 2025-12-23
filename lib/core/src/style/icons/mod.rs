//! Extensible icon system with Nerd Font, Unicode, and ASCII variants
//!
//! # Architecture
//!
//! The icon system uses a provider-based architecture for extensibility:
//! - [`IconProvider`] trait - Plugins implement this to register custom icons
//! - [`IconRegistry`] - Singleton that holds providers and queries icons
//! - [`IconSet`] - Selects which icon variant to use (Nerd/Unicode/ASCII)
//!
//! # Usage
//!
//! ```rust,ignore
//! use reovim_core::style::icons::{registry, IconSet};
//!
//! // Get a file icon
//! let icon = registry().read().unwrap().file_icon("rs");
//!
//! // Change icon set
//! registry().write().unwrap().set_icon_set(IconSet::Ascii);
//! ```

pub mod file_icons;
pub mod registry;
pub mod ui_icons;

pub use registry::{IconProvider, IconRegistry, registry};

/// Icon set variant selection
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IconSet {
    /// Nerd Fonts - Rich icons (requires Nerd Font installed)
    #[default]
    Nerd,
    /// Standard Unicode symbols
    Unicode,
    /// ASCII fallback for basic terminals
    Ascii,
}

/// Icon definition with variants for each icon set
#[derive(Debug, Clone, Copy)]
pub struct IconDef {
    /// Nerd Font icon
    pub nerd: &'static str,
    /// Unicode symbol
    pub unicode: &'static str,
    /// ASCII fallback
    pub ascii: &'static str,
}

impl IconDef {
    /// Create a new icon definition
    #[must_use]
    pub const fn new(nerd: &'static str, unicode: &'static str, ascii: &'static str) -> Self {
        Self {
            nerd,
            unicode,
            ascii,
        }
    }

    /// Get the icon string for the specified icon set
    #[must_use]
    pub const fn get(&self, set: IconSet) -> &'static str {
        match set {
            IconSet::Nerd => self.nerd,
            IconSet::Unicode => self.unicode,
            IconSet::Ascii => self.ascii,
        }
    }
}

/// Macro for easy icon definition
///
/// # Example
///
/// ```rust,ignore
/// use reovim_core::icon;
///
/// const RUST_ICON: IconDef = icon!("", "🦀", "rs");
/// ```
#[macro_export]
macro_rules! icon {
    ($nerd:literal, $unicode:literal, $ascii:literal) => {
        $crate::style::icons::IconDef::new($nerd, $unicode, $ascii)
    };
}

/// Spinner definition with frame variants for each icon set
#[derive(Debug, Clone, Copy)]
pub struct SpinnerDef {
    /// Nerd Font spinner frames
    pub nerd: &'static [&'static str],
    /// Unicode spinner frames
    pub unicode: &'static [&'static str],
    /// ASCII spinner frames
    pub ascii: &'static [&'static str],
}

impl SpinnerDef {
    /// Get the spinner frames for the specified icon set
    #[must_use]
    pub const fn get(&self, set: IconSet) -> &'static [&'static str] {
        match set {
            IconSet::Nerd => self.nerd,
            IconSet::Unicode => self.unicode,
            IconSet::Ascii => self.ascii,
        }
    }
}
