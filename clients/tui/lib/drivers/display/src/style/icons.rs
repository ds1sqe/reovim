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
}
