//! Style system extension for themes and icons.
//!
//! This module extends the display driver with theme management and
//! an extensible icon system.
//!
//! # Architecture
//!
//! The style system follows mechanism vs policy separation:
//!
//! - **Mechanism** (this module): Defines `IconProvider`, the
//!   `StyledTheme` super-trait that turns the registry's slim
//!   [`ThemeProvider`] into a `Style`-aware view, and the
//!   [`StyledThemeManagerExt`] extension trait that adds
//!   `get_style(group) -> Style` to the slim
//!   [`ThemeManager`].
//! - **Policy** (server/modules): Server decides which theme to use,
//!   modules provide custom icon mappings.
//!
//! # Theme Surface (post-#775 split)
//!
//! The slim trait/registry surface — [`BuiltinTheme`],
//! [`ThemeProvider`], [`ThemeManager`], [`SharedThemeManager`],
//! [`ThemeLoader`], [`ThemeInfo`], [`ThemeError`] — lives in
//! `reovim-driver-display-registry::theme`. This module re-exports
//! those types under their pre-split paths so existing client-side
//! consumers keep compiling.
//!
//! The display tier adds `Style`-aware extensions:
//!
//! - [`StyledTheme`] — super-trait of `ThemeProvider` that exposes
//!   `get_style(group) -> Option<Style>` and `default_style() -> Style`.
//!   `SimpleBuiltinTheme` and `FileTheme` implement it.
//! - [`StyledThemeManagerExt`] — extension on the slim manager that
//!   walks the current `ThemeProvider`'s hierarchical groups and
//!   resolves a `Style`.
//! - [`install_theme_factory`] — registers the display tier's
//!   [`DisplayThemeFactory`] in the registry crate's process-global
//!   factory slot. Must be called before
//!   `BuiltinTheme::Dark.load()` is expected to return a
//!   `Style`-aware provider.
//!
//! # Icon System
//!
//! ```ignore
//! use reovim_driver_display::style::{
//!     IconRegistry, IconSet, BuiltinFileIconProvider,
//! };
//!
//! let mut registry = IconRegistry::new(IconSet::Nerd);
//! registry.register(Box::new(BuiltinFileIconProvider));
//! let icon = registry.file_icon("main.rs", Some("rs"));
//! ```

mod builtin;
pub mod factory;
pub mod file;
pub mod groups;
mod icons;
mod manager;
mod registry;
mod theme;

pub use {
    factory::{DisplayThemeFactory, install_theme_factory},
    file::FileTheme,
    groups::ALL_GROUPS,
    icons::{
        BuiltinFileIconProvider, IconDef, IconProvider, IconRegistry, IconSet, file_icons, ui_icons,
    },
    manager::{StyledDowncaster, StyledThemeManagerExt, register_styled_downcaster},
    registry::StyleGroupRegistry,
    theme::StyledTheme,
};

// Re-export the slim trait/registry surface from the server-tier
// `reovim-driver-display-registry` crate. After #775 these types
// canonically live there; this re-export keeps the pre-split paths
// (`reovim_driver_display::style::ThemeManager`, etc.) compiling for
// client-side consumers.
pub use reovim_driver_display_registry::theme::{
    BuiltinTheme, SharedThemeManager, ThemeError, ThemeInfo, ThemeLoader, ThemeManager,
    ThemeProvider,
};
