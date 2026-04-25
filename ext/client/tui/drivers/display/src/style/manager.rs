//! Display-tier extension for the slim
//! [`ThemeManager`](reovim_driver_display_registry::theme::ThemeManager).
//!
//! The slim manager owns `Arc<dyn ThemeProvider>` and exposes only
//! the theme name and provider handle. The display side resolves a
//! highlight group to a concrete [`Style`] via the [`StyledTheme`]
//! super-trait, recovering the styled view through an `Any` downcast
//! on the slim manager's `current_theme()` provider.
//!
//! Earlier revisions of this manager carried tier-1 user overrides
//! and tier-3 module defaults backed by `StyleGroupRegistry`. Neither
//! tier had any production callers (verified by `rg`), so the
//! display-tier surface here was simplified to a single
//! `get_style(group) -> Style` that walks the current theme's
//! hierarchical groups (`keyword.control` → `keyword`) before
//! falling back to the theme default.
//!
//! ## Extending the styled-provider downcast list
//!
//! [`current_styled_theme`](StyledThemeManagerExt::current_styled_theme)
//! recovers `&dyn StyledTheme` from `&dyn ThemeProvider` via `Any`
//! downcast. The built-in concrete types
//! (`SimpleBuiltinTheme`, `FileTheme`) are recognised by default;
//! external code (notably the TUI's render-engine-bridge tests) may
//! register additional downcasters via
//! [`register_styled_downcaster`].
//!
//! [`StyledTheme`]: super::StyledTheme

use std::{
    any::Any,
    sync::{Mutex, OnceLock},
};

use reovim_driver_display_registry::theme::ThemeManager;

use crate::highlight::Style;

use super::theme::StyledTheme;

/// Function pointer type for projecting `&dyn Any` (the erased view
/// of the current `&dyn ThemeProvider`) into `&dyn StyledTheme` when
/// the concrete type matches. Returns `None` for type mismatches.
pub type StyledDowncaster = fn(&dyn Any) -> Option<&dyn StyledTheme>;

/// Process-global registry of downcasters that
/// [`StyledThemeManagerExt::current_styled_theme`] consults to
/// recover the styled view of an arbitrary `Arc<dyn ThemeProvider>`.
fn downcaster_registry() -> &'static Mutex<Vec<StyledDowncaster>> {
    static REGISTRY: OnceLock<Mutex<Vec<StyledDowncaster>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register a downcaster that [`StyledThemeManagerExt::current_styled_theme`]
/// consults when projecting a slim `&dyn ThemeProvider` into a
/// `&dyn StyledTheme`.
///
/// Each downcaster is a `fn(&dyn Any) -> Option<&dyn StyledTheme>`
/// that pattern-matches the concrete type and returns `Some(self)`
/// on hit. The display crate auto-registers downcasters for the
/// built-in `SimpleBuiltinTheme` and `FileTheme` impls; external
/// `StyledTheme` impls (e.g. test fakes) call this to opt in.
///
/// Idempotent in spirit: calling twice with the same `fn` pointer
/// adds a duplicate entry that is harmless because the first match
/// short-circuits the lookup.
pub fn register_styled_downcaster(downcaster: StyledDowncaster) {
    if let Ok(mut registry) = downcaster_registry().lock() {
        registry.push(downcaster);
    }
}

fn lookup_styled_downcaster(any: &dyn Any) -> Option<&dyn StyledTheme> {
    if let Ok(registry) = downcaster_registry().lock() {
        for downcaster in registry.iter() {
            if let Some(styled) = downcaster(any) {
                return Some(styled);
            }
        }
    }
    None
}

fn ensure_builtin_downcasters() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        register_styled_downcaster(|any| {
            any.downcast_ref::<super::theme::SimpleBuiltinTheme>()
                .map(|t| t as &dyn StyledTheme)
        });
        register_styled_downcaster(|any| {
            any.downcast_ref::<super::file::FileTheme>()
                .map(|t| t as &dyn StyledTheme)
        });
    });
}

/// Display-tier extension trait that adds `Style`-aware lookups to
/// the slim [`ThemeManager`].
///
/// Bring this trait into scope to call `manager.get_style(group)`:
///
/// ```ignore
/// use reovim_driver_display::style::StyledThemeManagerExt as _;
/// let style = manager.get_style("keyword");
/// ```
///
/// The lookup walks the dot-separated hierarchy: `"keyword.control"`
/// resolves through `"keyword.control"` → `"keyword"` → theme default.
/// If the current `ThemeProvider` cannot be downcast to a
/// [`StyledTheme`] (e.g. it is the registry's stub fallback used in
/// pure-server deployments), the lookup returns `Style::default()`.
pub trait StyledThemeManagerExt {
    /// Recover the `Style`-aware view of the current theme, if the
    /// concrete provider implements [`StyledTheme`].
    fn current_styled_theme(&self) -> Option<&dyn StyledTheme>;

    /// Get the style for a highlight group with hierarchical
    /// fallback.
    fn get_style(&self, group: &str) -> Style;
}

impl StyledThemeManagerExt for ThemeManager {
    fn current_styled_theme(&self) -> Option<&dyn StyledTheme> {
        ensure_builtin_downcasters();
        // `current_theme()` returns &Arc<dyn ThemeProvider>; deref
        // through `Arc` to a `&dyn ThemeProvider` and project to
        // `&dyn Any` via the `AsAny` super-trait.
        let provider: &dyn reovim_driver_display_registry::theme::ThemeProvider =
            &**self.current_theme();
        let any = provider.as_any();
        lookup_styled_downcaster(any)
    }

    fn get_style(&self, group: &str) -> Style {
        let Some(styled) = self.current_styled_theme() else {
            return Style::default();
        };

        if let Some(style) = styled.get_style(group) {
            return style;
        }

        let mut current = group;
        while let Some((parent, _)) = current.rsplit_once('.') {
            if let Some(style) = styled.get_style(parent) {
                return style;
            }
            current = parent;
        }

        styled.default_style()
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
