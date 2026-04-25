//! Theme provider trait (server-tier slice).
//!
//! This trait is intentionally opaque: it carries the theme name and a
//! `&dyn Any` projection. The display tier adds a `StyledTheme`
//! super-trait that returns `Style` for a given highlight group;
//! downcast from `&dyn ThemeProvider` to the styled view goes through
//! [`ThemeProvider::as_any`].
//!
//! # Why no blanket impl
//!
//! `as_any` is a regular method on `ThemeProvider` rather than a
//! blanket impl on a separate `AsAny` trait. The blanket form
//! (`impl<T: Any> AsAny for T`) would also satisfy
//! `Arc<dyn ThemeProvider>: AsAny`, and Rust's method-resolution
//! rules pick that direct match over a vtable dispatch through
//! `dyn ThemeProvider`. The result was that `theme.as_any()` on an
//! `Arc<dyn ThemeProvider>` returned a `&dyn Any` pointing at the
//! `Arc` itself, breaking downstream `downcast_ref::<Concrete>()`.
//! Folding `as_any` into `ThemeProvider` forces every concrete impl
//! to provide its own `&dyn Any` view, and method calls on the trait
//! object dispatch through the vtable to the concrete type's body.

use std::any::Any;

/// Theme provider — server-tier slice.
///
/// Every concrete theme implementation lives in the display tier and
/// implements both this trait and the display-side `StyledTheme`
/// super-trait. The server tier holds `Arc<dyn ThemeProvider>` and
/// only ever asks for `name()`; rendering code on the display side
/// downcasts via [`ThemeProvider::as_any`] to recover the
/// `Style`-aware view.
///
/// # Example
///
/// ```ignore
/// use std::{any::Any, sync::Arc};
/// use reovim_driver_display_registry::theme::ThemeProvider;
///
/// struct NamedTheme(&'static str);
/// impl ThemeProvider for NamedTheme {
///     fn name(&self) -> &str { self.0 }
///     fn as_any(&self) -> &dyn Any { self }
/// }
///
/// let theme: Arc<dyn ThemeProvider> = Arc::new(NamedTheme("custom"));
/// assert_eq!(theme.name(), "custom");
/// ```
pub trait ThemeProvider: Send + Sync + 'static {
    /// Get the theme name.
    fn name(&self) -> &str;

    /// Project `&Self` into `&dyn Any` so the display tier can recover
    /// the `Style`-aware view of a registered theme provider via
    /// `downcast_ref`. Standard impl is `fn as_any(&self) -> &dyn Any { self }`.
    fn as_any(&self) -> &dyn Any;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
