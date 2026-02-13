//! Icon provider system for extensible file and UI icons.
//!
//! Provides a registry-based icon system with priority-based provider lookup
//! and fallback variants for different terminal capabilities.

/// Icon definition with fallback variants.
///
/// Each icon has three variants for different terminal capabilities:
/// - `nerd`: Nerd Font icon (richest, requires Nerd Fonts)
/// - `unicode`: Standard Unicode symbol
/// - `ascii`: ASCII fallback for basic terminals
#[derive(Debug, Clone, Copy)]
pub struct IconDef {
    /// Nerd Font icon (default, richest)
    pub nerd: &'static str,
    /// Unicode symbol fallback
    pub unicode: &'static str,
    /// ASCII fallback for basic terminals
    pub ascii: &'static str,
}

impl IconDef {
    /// Create a new icon definition.
    #[must_use]
    pub const fn new(nerd: &'static str, unicode: &'static str, ascii: &'static str) -> Self {
        Self {
            nerd,
            unicode,
            ascii,
        }
    }

    /// Get the icon string for the given icon set.
    #[must_use]
    pub const fn get(&self, set: IconSet) -> &'static str {
        match set {
            IconSet::Nerd => self.nerd,
            IconSet::Unicode => self.unicode,
            IconSet::Ascii => self.ascii,
        }
    }
}

/// Macro for creating icon definitions.
///
/// # Example
///
/// ```ignore
/// const FILE_ICON: IconDef = icon!("󰈙 ", "📄 ", "F ");
/// ```
#[macro_export]
macro_rules! icon {
    ($nerd:literal, $unicode:literal, $ascii:literal) => {
        $crate::style::IconDef::new($nerd, $unicode, $ascii)
    };
}

/// Icon set selection based on terminal capability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IconSet {
    /// Nerd Fonts (default, richest icons)
    #[default]
    Nerd,
    /// Standard Unicode symbols
    Unicode,
    /// ASCII characters (maximum compatibility)
    Ascii,
}

/// Common file icons.
pub mod file_icons {
    use super::IconDef;

    /// Default file icon
    pub const FILE: IconDef = IconDef::new("󰈙 ", "📄 ", "F ");

    /// Folder icon
    pub const FOLDER: IconDef = IconDef::new("󰉋 ", "📁 ", "D ");

    /// Open folder icon
    pub const FOLDER_OPEN: IconDef = IconDef::new("󰝰 ", "📂 ", "D ");

    /// Rust file icon
    pub const RUST: IconDef = IconDef::new(" ", "🦀 ", "rs");

    /// JavaScript file icon
    pub const JAVASCRIPT: IconDef = IconDef::new("󰌞 ", "JS ", "js");

    /// TypeScript file icon
    pub const TYPESCRIPT: IconDef = IconDef::new("󰛦 ", "TS ", "ts");

    /// Python file icon
    pub const PYTHON: IconDef = IconDef::new("󰌠 ", "🐍 ", "py");

    /// Markdown file icon
    pub const MARKDOWN: IconDef = IconDef::new("󰍔 ", "📝 ", "md");

    /// JSON file icon
    pub const JSON: IconDef = IconDef::new("󰘦 ", "{ }", "{}");

    /// TOML file icon
    pub const TOML: IconDef = IconDef::new(" ", "⚙ ", "tm");

    /// YAML file icon
    pub const YAML: IconDef = IconDef::new(" ", "⚙ ", "ym");

    /// Git icon
    pub const GIT: IconDef = IconDef::new("󰊢 ", "⎇ ", "G ");
}

/// UI element icons.
pub mod ui_icons {
    use super::IconDef;

    /// Error/diagnostic icon
    pub const ERROR: IconDef = IconDef::new("󰅚 ", "✗ ", "E ");

    /// Warning icon
    pub const WARNING: IconDef = IconDef::new("󰀪 ", "⚠ ", "W ");

    /// Info icon
    pub const INFO: IconDef = IconDef::new("󰋽 ", "ℹ ", "I ");

    /// Hint icon
    pub const HINT: IconDef = IconDef::new("󰌶 ", "💡 ", "H ");

    /// Modified indicator
    pub const MODIFIED: IconDef = IconDef::new("● ", "● ", "* ");

    /// Readonly indicator
    pub const READONLY: IconDef = IconDef::new("󰌾 ", "🔒 ", "R ");

    /// Search icon
    pub const SEARCH: IconDef = IconDef::new("󰍉 ", "🔍 ", "? ");

    /// Close/dismiss icon
    pub const CLOSE: IconDef = IconDef::new("󰅖 ", "✕ ", "x ");

    /// Check/success icon
    pub const CHECK: IconDef = IconDef::new("󰄬 ", "✓ ", "v ");

    /// Arrow right
    pub const ARROW_RIGHT: IconDef = IconDef::new("󰁔 ", "→ ", "> ");

    /// Arrow down
    pub const ARROW_DOWN: IconDef = IconDef::new("󰁆 ", "↓ ", "v ");
}

/// Provider trait for extensible icons.
///
/// Plugins implement this trait to provide custom icons for specific
/// file types, languages, or UI elements.
pub trait IconProvider: Send + Sync {
    /// Provider name for debugging.
    fn name(&self) -> &'static str;

    /// Provider priority (higher = checked first).
    ///
    /// Built-in providers use priority 0.
    /// Plugin providers should use 100+ to override.
    fn priority(&self) -> u8;

    /// Get icon for a file by name and extension.
    ///
    /// Returns `None` to fall through to next provider.
    fn file_icon(&self, filename: &str, extension: Option<&str>) -> Option<&'static IconDef>;

    /// Get icon for a directory.
    ///
    /// Returns `None` to fall through to next provider.
    fn dir_icon(&self, dirname: &str) -> Option<&'static IconDef>;

    /// Get icon by kind identifier.
    ///
    /// Used for completion kinds, diagnostic types, etc.
    /// Returns `None` to fall through to next provider.
    fn kind_icon(&self, kind: &str) -> Option<&'static IconDef>;
}

/// Icon registry with priority-based provider lookup.
///
/// Providers are queried in priority order (highest first).
/// The first provider to return `Some` wins.
pub struct IconRegistry {
    providers: Vec<Box<dyn IconProvider>>,
    icon_set: IconSet,
}

impl IconRegistry {
    /// Create a new icon registry with the given icon set.
    #[must_use]
    pub fn new(icon_set: IconSet) -> Self {
        Self {
            providers: Vec::new(),
            icon_set,
        }
    }

    /// Register a provider.
    ///
    /// Providers are automatically sorted by priority (highest first).
    pub fn register(&mut self, provider: Box<dyn IconProvider>) {
        self.providers.push(provider);
        self.providers
            .sort_by_key(|p| std::cmp::Reverse(p.priority()));
    }

    /// Set the icon set for output.
    pub const fn set_icon_set(&mut self, set: IconSet) {
        self.icon_set = set;
    }

    /// Get the current icon set.
    #[must_use]
    pub const fn icon_set(&self) -> IconSet {
        self.icon_set
    }

    /// Get icon for a file.
    #[must_use]
    pub fn file_icon(&self, filename: &str, extension: Option<&str>) -> &'static str {
        for provider in &self.providers {
            if let Some(icon) = provider.file_icon(filename, extension) {
                return icon.get(self.icon_set);
            }
        }
        file_icons::FILE.get(self.icon_set)
    }

    /// Get icon for a directory.
    #[must_use]
    pub fn dir_icon(&self, dirname: &str) -> &'static str {
        for provider in &self.providers {
            if let Some(icon) = provider.dir_icon(dirname) {
                return icon.get(self.icon_set);
            }
        }
        file_icons::FOLDER.get(self.icon_set)
    }

    /// Get icon by kind.
    #[must_use]
    pub fn kind_icon(&self, kind: &str) -> &'static str {
        for provider in &self.providers {
            if let Some(icon) = provider.kind_icon(kind) {
                return icon.get(self.icon_set);
            }
        }
        ""
    }

    /// Get number of registered providers.
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

impl Default for IconRegistry {
    fn default() -> Self {
        Self::new(IconSet::default())
    }
}

/// Built-in icon provider based on file extension.
pub struct BuiltinFileIconProvider;

impl IconProvider for BuiltinFileIconProvider {
    fn name(&self) -> &'static str {
        "builtin-file"
    }

    fn priority(&self) -> u8 {
        0
    }

    fn file_icon(&self, _filename: &str, extension: Option<&str>) -> Option<&'static IconDef> {
        let ext = extension?;
        match ext {
            "rs" => Some(&file_icons::RUST),
            "js" | "mjs" | "cjs" => Some(&file_icons::JAVASCRIPT),
            "ts" | "mts" | "cts" => Some(&file_icons::TYPESCRIPT),
            "py" | "pyi" => Some(&file_icons::PYTHON),
            "md" | "markdown" => Some(&file_icons::MARKDOWN),
            "json" => Some(&file_icons::JSON),
            "toml" => Some(&file_icons::TOML),
            "yaml" | "yml" => Some(&file_icons::YAML),
            _ => None,
        }
    }

    fn dir_icon(&self, dirname: &str) -> Option<&'static IconDef> {
        match dirname {
            ".git" => Some(&file_icons::GIT),
            _ => None,
        }
    }

    fn kind_icon(&self, kind: &str) -> Option<&'static IconDef> {
        match kind {
            "error" => Some(&ui_icons::ERROR),
            "warning" | "warn" => Some(&ui_icons::WARNING),
            "info" | "information" => Some(&ui_icons::INFO),
            "hint" => Some(&ui_icons::HINT),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_def_get() {
        let icon = IconDef::new("N", "U", "A");

        assert_eq!(icon.get(IconSet::Nerd), "N");
        assert_eq!(icon.get(IconSet::Unicode), "U");
        assert_eq!(icon.get(IconSet::Ascii), "A");
    }

    #[test]
    fn test_icon_registry_default() {
        let registry = IconRegistry::default();
        assert_eq!(registry.icon_set(), IconSet::Nerd);
        assert_eq!(registry.provider_count(), 0);
    }

    #[test]
    fn test_icon_registry_with_provider() {
        let mut registry = IconRegistry::new(IconSet::Nerd);
        registry.register(Box::new(BuiltinFileIconProvider));

        assert_eq!(registry.provider_count(), 1);

        // Test file icon lookup
        let rust_icon = registry.file_icon("main.rs", Some("rs"));
        assert_eq!(rust_icon, file_icons::RUST.nerd);

        // Unknown extension falls back to default
        let unknown_icon = registry.file_icon("file.xyz", Some("xyz"));
        assert_eq!(unknown_icon, file_icons::FILE.nerd);
    }

    #[test]
    fn test_builtin_provider_file_icons() {
        let provider = BuiltinFileIconProvider;

        assert!(provider.file_icon("test.rs", Some("rs")).is_some());
        assert!(provider.file_icon("test.py", Some("py")).is_some());
        assert!(provider.file_icon("test.xyz", Some("xyz")).is_none());
    }

    #[test]
    fn test_builtin_provider_kind_icons() {
        let provider = BuiltinFileIconProvider;

        assert!(provider.kind_icon("error").is_some());
        assert!(provider.kind_icon("warning").is_some());
        assert!(provider.kind_icon("unknown").is_none());
    }

    // =========================================================================
    // Extended icon tests
    // =========================================================================

    #[test]
    fn test_icon_set_default() {
        assert_eq!(IconSet::default(), IconSet::Nerd);
    }

    #[test]
    fn test_icon_registry_file_icon_no_providers() {
        let registry = IconRegistry::new(IconSet::Nerd);
        // No providers, should fall back to default file icon
        let icon = registry.file_icon("test.rs", Some("rs"));
        assert_eq!(icon, file_icons::FILE.nerd);
    }

    #[test]
    fn test_icon_registry_dir_icon() {
        let mut registry = IconRegistry::new(IconSet::Nerd);
        registry.register(Box::new(BuiltinFileIconProvider));

        // .git directory
        let icon = registry.dir_icon(".git");
        assert_eq!(icon, file_icons::GIT.nerd);

        // Unknown directory falls back to default folder
        let icon = registry.dir_icon("src");
        assert_eq!(icon, file_icons::FOLDER.nerd);
    }

    #[test]
    fn test_icon_registry_dir_icon_no_providers() {
        let registry = IconRegistry::new(IconSet::Unicode);
        let icon = registry.dir_icon("any");
        assert_eq!(icon, file_icons::FOLDER.unicode);
    }

    #[test]
    fn test_icon_registry_kind_icon() {
        let mut registry = IconRegistry::new(IconSet::Nerd);
        registry.register(Box::new(BuiltinFileIconProvider));

        let icon = registry.kind_icon("error");
        assert_eq!(icon, ui_icons::ERROR.nerd);

        let icon = registry.kind_icon("warning");
        assert_eq!(icon, ui_icons::WARNING.nerd);

        let icon = registry.kind_icon("info");
        assert_eq!(icon, ui_icons::INFO.nerd);

        let icon = registry.kind_icon("hint");
        assert_eq!(icon, ui_icons::HINT.nerd);

        // Unknown kind returns empty
        let icon = registry.kind_icon("unknown_kind");
        assert_eq!(icon, "");
    }

    #[test]
    fn test_icon_registry_kind_icon_no_providers() {
        let registry = IconRegistry::new(IconSet::Nerd);
        let icon = registry.kind_icon("error");
        assert_eq!(icon, "");
    }

    #[test]
    fn test_icon_registry_set_icon_set() {
        let mut registry = IconRegistry::new(IconSet::Nerd);
        registry.register(Box::new(BuiltinFileIconProvider));

        // Initially Nerd
        let icon = registry.file_icon("test.rs", Some("rs"));
        assert_eq!(icon, file_icons::RUST.nerd);

        // Switch to Unicode
        registry.set_icon_set(IconSet::Unicode);
        let icon = registry.file_icon("test.rs", Some("rs"));
        assert_eq!(icon, file_icons::RUST.unicode);

        // Switch to ASCII
        registry.set_icon_set(IconSet::Ascii);
        let icon = registry.file_icon("test.rs", Some("rs"));
        assert_eq!(icon, file_icons::RUST.ascii);
    }

    #[test]
    fn test_builtin_provider_all_file_extensions() {
        let provider = BuiltinFileIconProvider;

        // All supported extensions
        assert!(provider.file_icon("test.rs", Some("rs")).is_some());
        assert!(provider.file_icon("test.js", Some("js")).is_some());
        assert!(provider.file_icon("test.mjs", Some("mjs")).is_some());
        assert!(provider.file_icon("test.cjs", Some("cjs")).is_some());
        assert!(provider.file_icon("test.ts", Some("ts")).is_some());
        assert!(provider.file_icon("test.mts", Some("mts")).is_some());
        assert!(provider.file_icon("test.cts", Some("cts")).is_some());
        assert!(provider.file_icon("test.py", Some("py")).is_some());
        assert!(provider.file_icon("test.pyi", Some("pyi")).is_some());
        assert!(provider.file_icon("test.md", Some("md")).is_some());
        assert!(
            provider
                .file_icon("test.markdown", Some("markdown"))
                .is_some()
        );
        assert!(provider.file_icon("test.json", Some("json")).is_some());
        assert!(provider.file_icon("test.toml", Some("toml")).is_some());
        assert!(provider.file_icon("test.yaml", Some("yaml")).is_some());
        assert!(provider.file_icon("test.yml", Some("yml")).is_some());

        // No extension should return None
        assert!(provider.file_icon("test", None).is_none());
    }

    #[test]
    fn test_builtin_provider_kind_icons_all() {
        let provider = BuiltinFileIconProvider;

        assert!(provider.kind_icon("error").is_some());
        assert!(provider.kind_icon("warning").is_some());
        assert!(provider.kind_icon("warn").is_some());
        assert!(provider.kind_icon("info").is_some());
        assert!(provider.kind_icon("information").is_some());
        assert!(provider.kind_icon("hint").is_some());
    }

    #[test]
    fn test_builtin_provider_metadata() {
        let provider = BuiltinFileIconProvider;
        assert_eq!(provider.name(), "builtin-file");
        assert_eq!(provider.priority(), 0);
    }

    #[test]
    fn test_builtin_provider_dir_icon_git() {
        let provider = BuiltinFileIconProvider;
        assert!(provider.dir_icon(".git").is_some());
        assert!(provider.dir_icon("src").is_none());
    }

    /// Test priority-based provider ordering.
    #[test]
    fn test_icon_registry_provider_priority() {
        struct HighPriorityProvider;

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl IconProvider for HighPriorityProvider {
            fn name(&self) -> &'static str {
                "high-priority"
            }
            fn priority(&self) -> u8 {
                100
            }
            fn file_icon(
                &self,
                _filename: &str,
                extension: Option<&str>,
            ) -> Option<&'static IconDef> {
                if extension == Some("rs") {
                    Some(&ui_icons::CHECK) // Return a different icon to verify priority
                } else {
                    None
                }
            }
            fn dir_icon(&self, _dirname: &str) -> Option<&'static IconDef> {
                None
            }
            fn kind_icon(&self, _kind: &str) -> Option<&'static IconDef> {
                None
            }
        }

        let mut registry = IconRegistry::new(IconSet::Nerd);
        registry.register(Box::new(BuiltinFileIconProvider));
        registry.register(Box::new(HighPriorityProvider));

        // High-priority provider should win for .rs files
        let icon = registry.file_icon("test.rs", Some("rs"));
        assert_eq!(icon, ui_icons::CHECK.nerd);

        // For other files, builtin should handle it
        let icon = registry.file_icon("test.py", Some("py"));
        assert_eq!(icon, file_icons::PYTHON.nerd);
    }

    #[test]
    fn test_file_icons_constants() {
        // Verify file icon constants exist and have all three variants
        let icons = [
            &file_icons::FILE,
            &file_icons::FOLDER,
            &file_icons::FOLDER_OPEN,
            &file_icons::RUST,
            &file_icons::JAVASCRIPT,
            &file_icons::TYPESCRIPT,
            &file_icons::PYTHON,
            &file_icons::MARKDOWN,
            &file_icons::JSON,
            &file_icons::TOML,
            &file_icons::YAML,
            &file_icons::GIT,
        ];

        for icon in icons {
            assert!(!icon.nerd.is_empty());
            assert!(!icon.unicode.is_empty());
            assert!(!icon.ascii.is_empty());
        }
    }

    #[test]
    fn test_ui_icons_constants() {
        let icons = [
            &ui_icons::ERROR,
            &ui_icons::WARNING,
            &ui_icons::INFO,
            &ui_icons::HINT,
            &ui_icons::MODIFIED,
            &ui_icons::READONLY,
            &ui_icons::SEARCH,
            &ui_icons::CLOSE,
            &ui_icons::CHECK,
            &ui_icons::ARROW_RIGHT,
            &ui_icons::ARROW_DOWN,
        ];

        for icon in icons {
            assert!(!icon.nerd.is_empty());
            assert!(!icon.unicode.is_empty());
            assert!(!icon.ascii.is_empty());
        }
    }
}
