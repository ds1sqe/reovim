use std::{any::Any, sync::Arc};

use super::ThemeProvider;

struct NamedTheme {
    name: &'static str,
}

impl ThemeProvider for NamedTheme {
    fn name(&self) -> &str {
        self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn theme_provider_reports_name() {
    let theme = NamedTheme { name: "test" };
    assert_eq!(theme.name(), "test");
}

#[test]
fn theme_provider_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn ThemeProvider>>();
}

#[test]
fn as_any_enables_downcast() {
    let theme: Arc<dyn ThemeProvider> = Arc::new(NamedTheme { name: "x" });
    let any: &dyn Any = theme.as_any();
    let recovered = any
        .downcast_ref::<NamedTheme>()
        .expect("downcast preserves concrete type");
    assert_eq!(recovered.name, "x");
}

#[test]
fn as_any_rejects_wrong_type() {
    struct OtherTheme;
    impl ThemeProvider for OtherTheme {
        fn name(&self) -> &'static str {
            "other"
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    let theme: Arc<dyn ThemeProvider> = Arc::new(NamedTheme { name: "x" });
    let any = theme.as_any();
    assert!(any.downcast_ref::<OtherTheme>().is_none());
}
