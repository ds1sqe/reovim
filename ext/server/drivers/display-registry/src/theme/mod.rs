//! Theme registry surface (server-tier slice).
//!
//! This module exposes the slim, `Style`-free theme types that the server
//! tier registers and consumes:
//!
//! - [`ThemeProvider`] — opaque trait with `name()` only. The display tier
//!   adds a `StyledTheme` super-trait with `get_style()`.
//! - [`BuiltinTheme`] — pure-data enum (Dark / Light / `TokyoNightOrange`).
//! - [`ThemeManager`] / [`SharedThemeManager`] — server-registered manager
//!   that holds the current `Arc<dyn ThemeProvider>`.
//! - [`ThemeLoader`] / [`ThemeInfo`] — file discovery + factory dispatch.
//! - [`ThemeError`] — TOML / IO error type with a boxed parse error
//!   (registry crate has no `toml` dependency).
//! - [`ThemeFactory`] / [`set_theme_factory`] / [`theme_factory`] —
//!   indirection so the display tier can inject `Style`-aware
//!   constructors at startup. When no factory is registered (e.g. in
//!   pure-server / subprocess deployments) loaders return stub
//!   providers that report only their name.

mod builtin;
mod error;
mod factory;
mod loader;
mod manager;
mod provider;

pub use {
    builtin::BuiltinTheme,
    error::ThemeError,
    factory::{ThemeFactory, set_theme_factory, theme_factory},
    loader::{ThemeInfo, ThemeLoader},
    manager::{SharedThemeManager, ThemeManager},
    provider::ThemeProvider,
};
