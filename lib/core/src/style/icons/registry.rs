//! Icon registry with extensible provider system
//!
//! The registry holds multiple [`IconProvider`] implementations and queries them
//! in priority order. Plugins can register their own providers to add custom icons.

use std::{
    cmp::Reverse,
    sync::{OnceLock, RwLock},
};

use super::IconSet;

/// Trait for icon providers - plugins implement this to register custom icons
///
/// Providers are queried in priority order (highest first). Return `None` to
/// fall through to the next provider.
///
/// # Example
///
/// ```rust,ignore
/// use reovim_core::style::icons::{IconProvider, IconSet, registry};
///
/// struct RustIconProvider;
///
/// impl IconProvider for RustIconProvider {
///     fn file_icon(&self, ext: &str, set: IconSet) -> Option<&'static str> {
///         if ext == "rs" {
///             return Some(match set {
///                 IconSet::Nerd => "",
///                 IconSet::Unicode => "🦀",
///                 IconSet::Ascii => "rs",
///             });
///         }
///         None
///     }
///
///     fn priority(&self) -> u32 { 100 }
/// }
///
/// // Register on plugin init
/// registry().write().unwrap().register_provider(Box::new(RustIconProvider));
/// ```
pub trait IconProvider: Send + Sync {
    /// Get icon for a file extension (return `None` to fall through)
    fn file_icon(&self, _ext: &str, _set: IconSet) -> Option<&'static str> {
        None
    }

    /// Get icon for a completion kind (return `None` to fall through)
    fn kind_icon(&self, _kind: &str, _set: IconSet) -> Option<&'static str> {
        None
    }

    /// Get icon by custom key (return `None` to fall through)
    fn icon(&self, _key: &str, _set: IconSet) -> Option<&'static str> {
        None
    }

    /// Get icon for a directory (open/closed state)
    fn dir_icon(&self, _expanded: bool, _set: IconSet) -> Option<&'static str> {
        None
    }

    /// Priority (higher = checked first). Built-in is 0, plugins default to 100.
    fn priority(&self) -> u32 {
        100
    }

    /// Provider name for debugging
    fn name(&self) -> &'static str {
        "unnamed"
    }
}

/// Icon registry - holds providers and queries icons
pub struct IconRegistry {
    icon_set: IconSet,
    providers: Vec<Box<dyn IconProvider>>,
}

impl Default for IconRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl IconRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            icon_set: IconSet::default(),
            providers: Vec::new(),
        }
    }

    /// Register a new icon provider
    ///
    /// Providers are automatically sorted by priority (highest first).
    pub fn register_provider(&mut self, provider: Box<dyn IconProvider>) {
        self.providers.push(provider);
        self.providers.sort_by_key(|p| Reverse(p.priority()));
    }

    /// Get the current icon set
    #[must_use]
    pub const fn icon_set(&self) -> IconSet {
        self.icon_set
    }

    /// Set the active icon set
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_icon_set(&mut self, set: IconSet) {
        self.icon_set = set;
    }

    /// Query file icon by extension
    ///
    /// Tries providers in priority order, returns default if none match.
    #[must_use]
    pub fn file_icon(&self, ext: &str) -> &'static str {
        let ext_lower = ext.to_lowercase();
        for provider in &self.providers {
            if let Some(icon) = provider.file_icon(&ext_lower, self.icon_set) {
                return icon;
            }
        }
        // Default file icon
        match self.icon_set {
            IconSet::Nerd => " ",
            IconSet::Unicode => "📄 ",
            IconSet::Ascii => "  ",
        }
    }

    /// Query directory icon (open/closed)
    #[must_use]
    pub fn dir_icon(&self, expanded: bool) -> &'static str {
        for provider in &self.providers {
            if let Some(icon) = provider.dir_icon(expanded, self.icon_set) {
                return icon;
            }
        }
        // Default directory icons
        if expanded {
            match self.icon_set {
                IconSet::Nerd => "󰝰 ",
                IconSet::Unicode => "📂 ",
                IconSet::Ascii => "v ",
            }
        } else {
            match self.icon_set {
                IconSet::Nerd => "󰉋 ",
                IconSet::Unicode => "📁 ",
                IconSet::Ascii => "> ",
            }
        }
    }

    /// Query completion kind icon
    #[must_use]
    pub fn kind_icon(&self, kind: &str) -> &'static str {
        for provider in &self.providers {
            if let Some(icon) = provider.kind_icon(kind, self.icon_set) {
                return icon;
            }
        }
        // Default kind icon
        match self.icon_set {
            IconSet::Nerd => " ",
            IconSet::Unicode => "• ",
            IconSet::Ascii => "* ",
        }
    }

    /// Query custom icon by key
    #[must_use]
    pub fn icon(&self, key: &str) -> &'static str {
        for provider in &self.providers {
            if let Some(icon) = provider.icon(key, self.icon_set) {
                return icon;
            }
        }
        // Default icon
        match self.icon_set {
            IconSet::Nerd => " ",
            IconSet::Unicode => "• ",
            IconSet::Ascii => "* ",
        }
    }

    /// Get number of registered providers
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

// Global singleton
static REGISTRY: OnceLock<RwLock<IconRegistry>> = OnceLock::new();

/// Get the global icon registry
///
/// The registry is lazily initialized with built-in icons on first access.
#[must_use]
pub fn registry() -> &'static RwLock<IconRegistry> {
    REGISTRY.get_or_init(|| {
        let mut reg = IconRegistry::new();
        // Register built-in providers
        reg.register_provider(Box::new(super::file_icons::BuiltinFileIconProvider));
        reg.register_provider(Box::new(super::ui_icons::BuiltinUiIconProvider));
        RwLock::new(reg)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider;

    impl IconProvider for TestProvider {
        fn file_icon(&self, ext: &str, _set: IconSet) -> Option<&'static str> {
            if ext == "test" {
                return Some("T");
            }
            None
        }

        fn priority(&self) -> u32 {
            200
        }

        fn name(&self) -> &'static str {
            "test"
        }
    }

    #[test]
    fn test_registry_provider_priority() {
        let mut reg = IconRegistry::new();
        reg.register_provider(Box::new(TestProvider));
        reg.register_provider(Box::new(super::super::file_icons::BuiltinFileIconProvider));

        // TestProvider has higher priority, so it should be checked first
        assert_eq!(reg.file_icon("test"), "T");
    }

    #[test]
    fn test_registry_fallback() {
        let reg = IconRegistry::new();
        // No providers, should return default
        let icon = reg.file_icon("unknown");
        assert!(!icon.is_empty());
    }

    #[test]
    fn test_icon_set_change() {
        let mut reg = IconRegistry::new();
        reg.register_provider(Box::new(super::super::file_icons::BuiltinFileIconProvider));

        reg.set_icon_set(IconSet::Nerd);
        let nerd_icon = reg.file_icon("rs");

        reg.set_icon_set(IconSet::Ascii);
        let ascii_icon = reg.file_icon("rs");

        // Icons should be different for different sets
        assert_ne!(nerd_icon, ascii_icon);
    }

    #[test]
    fn test_global_registry_file_icons() {
        // Test the global registry returns correct icons
        let (rs_icon, toml_icon, md_icon, unknown_icon) = {
            let reg = registry().read().unwrap();
            (
                reg.file_icon("rs"),
                reg.file_icon("toml"),
                reg.file_icon("md"),
                reg.file_icon("xyz123"),
            )
        };

        // All known extensions should return specific icons, not the default
        assert_eq!(rs_icon, " ", "rs should return Rust icon");
        assert_eq!(toml_icon, " ", "toml should return toml icon");
        assert_eq!(md_icon, " ", "md should return markdown icon");

        // Unknown extensions should return the default
        assert_eq!(unknown_icon, " ", "unknown should return default icon");
    }
}
