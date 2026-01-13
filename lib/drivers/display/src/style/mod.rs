//! Style system extension for themes and icons.
//!
//! This module extends the display driver with theme management and
//! an extensible icon system.
//!
//! # Architecture
//!
//! The style system follows mechanism vs policy separation:
//!
//! - **Mechanism** (this module): Defines `ThemeProvider`, `IconProvider` traits
//!   and provides `ThemeManager`, `IconRegistry` implementations.
//! - **Policy** (runner/plugins): Runner decides which theme to use, plugins
//!   provide custom icon mappings.
//!
//! # Theme System
//!
//! ```ignore
//! use reovim_driver_display::style::{ThemeManager, BuiltinTheme};
//!
//! // Create theme manager (runner owns this)
//! let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());
//!
//! // Override specific styles
//! manager.set_override("keyword", keyword_style);
//!
//! // Get style for rendering
//! let style = manager.get_style("keyword");
//! ```
//!
//! # Icon System
//!
//! ```ignore
//! use reovim_driver_display::style::{IconRegistry, IconSet, BuiltinFileIconProvider};
//!
//! // Create registry with default icon set
//! let mut registry = IconRegistry::new(IconSet::Nerd);
//!
//! // Register built-in provider
//! registry.register(Box::new(BuiltinFileIconProvider));
//!
//! // Get icon for file
//! let icon = registry.file_icon("main.rs", Some("rs"));
//! ```
//!
//! # Note
//!
//! The `HighlightGroup` enum (~70 variants) remains in `reovim-core::highlight`
//! for now. This theme system uses string-based group names for flexibility.
//! Future Phase 6 may migrate `HighlightGroup` to this driver.

mod icons;
mod manager;
mod theme;

pub use {
    icons::{
        BuiltinFileIconProvider, IconDef, IconProvider, IconRegistry, IconSet, file_icons, ui_icons,
    },
    manager::ThemeManager,
    theme::{BuiltinTheme, CoreThemeAdapter, ThemeProvider},
};
