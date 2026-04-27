//! Tests for [`ThemeFactory`] and the process-global factory slot.
//!
//! Note on test isolation: `set_theme_factory` writes to a process-wide
//! `OnceLock`, which is shared across `cargo test` threads. Only one
//! test in this module touches the slot
//! (`factory_singleton_set_once_wins`); the others exercise trait
//! object safety without reading or writing the global.

use std::sync::Arc;

use super::{
    super::{BuiltinTheme, ThemeError, provider::ThemeProvider},
    ThemeFactory, set_theme_factory, theme_factory,
};

/// Test factory installed as the singleton: its `load_builtin` honors
/// the variant's stable name so non-factory tests in sibling modules
/// (e.g. loader tests reaching builtin fallback) see the expected
/// names. `load_file` returns the per-instance `label` so the
/// singleton test can prove the registered factory is actually being
/// invoked.
struct LabeledFactory {
    label: &'static str,
}

impl ThemeFactory for LabeledFactory {
    fn load_builtin(&self, theme: BuiltinTheme) -> Arc<dyn ThemeProvider> {
        Arc::new(NamedProvider { name: theme.name() })
    }

    fn load_file(&self, _name: &str, _content: &str) -> Result<Arc<dyn ThemeProvider>, ThemeError> {
        Ok(Arc::new(NamedProvider { name: self.label }))
    }
}

struct NamedProvider {
    name: &'static str,
}

impl ThemeProvider for NamedProvider {
    fn name(&self) -> &str {
        self.name
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn theme_factory_is_object_safe() {
    let factory: Arc<dyn ThemeFactory> = Arc::new(LabeledFactory { label: "first" });
    let theme = factory.load_builtin(BuiltinTheme::Dark);
    assert_eq!(theme.name(), BuiltinTheme::Dark.name());
}

#[test]
fn theme_factory_load_file_returns_provider() {
    let factory = LabeledFactory { label: "fileful" };
    let theme = factory.load_file("ignored", "ignored").unwrap();
    assert_eq!(theme.name(), "fileful");
}

#[test]
fn theme_factory_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn ThemeFactory>>();
}

#[test]
fn factory_singleton_set_once_wins() {
    // This test owns the process-global `FACTORY` slot for the
    // duration of `cargo test -p reovim-driver-display-registry`. If
    // a second test tries to read or write it, the assertions below
    // race; that's why no other test in this module touches the
    // global, and the LabeledFactory installed here proxies
    // `load_builtin` to the variant's name (so other tests in this
    // crate that reach `BuiltinTheme::load()` still see expected
    // names).

    set_theme_factory(Arc::new(LabeledFactory { label: "winner" }));
    let installed = theme_factory().expect("factory installed");

    // `load_file` carries the per-instance label so we can prove the
    // *registered* factory is being invoked.
    let from_file = installed.load_file("any", "any").expect("load_file ok");
    assert_eq!(from_file.name(), "winner");

    // Subsequent set is a no-op (OnceLock semantics).
    set_theme_factory(Arc::new(LabeledFactory { label: "loser" }));
    let still_installed = theme_factory().expect("factory still present");
    let from_file_again = still_installed
        .load_file("any", "any")
        .expect("load_file ok");
    assert_eq!(
        from_file_again.name(),
        "winner",
        "OnceLock::set returns Err on second call; first factory wins"
    );
}
