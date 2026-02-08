//! Filetype detection service for the VFS driver.
//!
//! Linux equivalent: File type detection in `fs/` (like magic number detection)
//!
//! This module provides filetype detection based on:
//! - File extension (e.g., `.rs` -> Rust)
//! - Exact filename (e.g., `Makefile` -> Make)
//!
//! # Design Philosophy
//!
//! This is a **mechanism** for detecting file types. Modules can use this
//! service to determine syntax highlighting, LSP server selection, etc.
//!
//! # Example
//!
//! ```
//! use reovim_driver_vfs::{FiletypeInfo, FiletypeRegistry, detect_filetype};
//!
//! // Using the global registry
//! if let Some(info) = detect_filetype("main.rs") {
//!     assert_eq!(info.id, "rust");
//!     assert_eq!(info.display_name, "Rust");
//! }
//!
//! // Create a custom registry
//! let mut registry = FiletypeRegistry::new();
//! registry.register_extension("custom", FiletypeInfo::new("custom", "Custom Lang"));
//! assert_eq!(registry.detect("file.custom").map(|i| i.id), Some("custom"));
//! ```

use std::{collections::HashMap, path::Path, sync::OnceLock};

/// Information about a detected filetype.
///
/// Contains metadata useful for syntax highlighting, icons, and LSP selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiletypeInfo {
    /// Unique identifier (e.g., "rust", "python").
    ///
    /// This ID is used for LSP server selection and treesitter grammar lookup.
    pub id: &'static str,
    /// Human-readable display name (e.g., "Rust", "Python").
    pub display_name: &'static str,
    /// Optional Nerd Font icon.
    pub icon: Option<&'static str>,
}

impl FiletypeInfo {
    /// Create a new filetype info without an icon.
    #[must_use]
    pub const fn new(id: &'static str, display_name: &'static str) -> Self {
        Self {
            id,
            display_name,
            icon: None,
        }
    }

    /// Create a new filetype info with an icon.
    #[must_use]
    pub const fn with_icon(
        id: &'static str,
        display_name: &'static str,
        icon: &'static str,
    ) -> Self {
        Self {
            id,
            display_name,
            icon: Some(icon),
        }
    }
}

/// Registry for filetype detection.
///
/// Maps file extensions and exact filenames to filetype information.
/// The registry is populated with common filetypes by default.
pub struct FiletypeRegistry {
    /// Extension to filetype mapping (lowercase keys).
    extension_map: HashMap<&'static str, FiletypeInfo>,
    /// Exact filename to filetype mapping.
    filename_map: HashMap<&'static str, FiletypeInfo>,
}

impl Default for FiletypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FiletypeRegistry {
    /// Create a new registry with default filetypes.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            extension_map: HashMap::new(),
            filename_map: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Create an empty registry without default filetypes.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            extension_map: HashMap::new(),
            filename_map: HashMap::new(),
        }
    }

    /// Register default filetypes.
    fn register_defaults(&mut self) {
        // Rust
        self.register_extension("rs", FiletypeInfo::with_icon("rust", "Rust", ""));

        // Python
        self.register_extension("py", FiletypeInfo::with_icon("python", "Python", ""));
        self.register_extension("pyi", FiletypeInfo::with_icon("python", "Python", ""));
        self.register_extension("pyw", FiletypeInfo::with_icon("python", "Python", ""));

        // JavaScript/TypeScript
        self.register_extension("js", FiletypeInfo::with_icon("javascript", "JavaScript", ""));
        self.register_extension("mjs", FiletypeInfo::with_icon("javascript", "JavaScript", ""));
        self.register_extension("cjs", FiletypeInfo::with_icon("javascript", "JavaScript", ""));
        self.register_extension("ts", FiletypeInfo::with_icon("typescript", "TypeScript", ""));
        self.register_extension("jsx", FiletypeInfo::with_icon("javascriptreact", "JSX", ""));
        self.register_extension("tsx", FiletypeInfo::with_icon("typescriptreact", "TSX", ""));

        // C/C++
        self.register_extension("c", FiletypeInfo::with_icon("c", "C", ""));
        self.register_extension("h", FiletypeInfo::with_icon("c", "C", ""));
        self.register_extension("cpp", FiletypeInfo::with_icon("cpp", "C++", ""));
        self.register_extension("cc", FiletypeInfo::with_icon("cpp", "C++", ""));
        self.register_extension("cxx", FiletypeInfo::with_icon("cpp", "C++", ""));
        self.register_extension("hpp", FiletypeInfo::with_icon("cpp", "C++", ""));
        self.register_extension("hxx", FiletypeInfo::with_icon("cpp", "C++", ""));

        // Go
        self.register_extension("go", FiletypeInfo::with_icon("go", "Go", ""));

        // Lua
        self.register_extension("lua", FiletypeInfo::with_icon("lua", "Lua", ""));

        // Shell
        self.register_extension("sh", FiletypeInfo::with_icon("bash", "Bash", ""));
        self.register_extension("bash", FiletypeInfo::with_icon("bash", "Bash", ""));
        self.register_extension("zsh", FiletypeInfo::with_icon("zsh", "Zsh", ""));
        self.register_extension("fish", FiletypeInfo::with_icon("fish", "Fish", ""));

        // Data formats
        self.register_extension("json", FiletypeInfo::with_icon("json", "JSON", ""));
        self.register_extension("toml", FiletypeInfo::with_icon("toml", "TOML", ""));
        self.register_extension("yaml", FiletypeInfo::with_icon("yaml", "YAML", ""));
        self.register_extension("yml", FiletypeInfo::with_icon("yaml", "YAML", ""));
        self.register_extension("xml", FiletypeInfo::with_icon("xml", "XML", ""));

        // Markup
        self.register_extension("md", FiletypeInfo::with_icon("markdown", "Markdown", ""));
        self.register_extension("markdown", FiletypeInfo::with_icon("markdown", "Markdown", ""));
        self.register_extension("html", FiletypeInfo::with_icon("html", "HTML", ""));
        self.register_extension("htm", FiletypeInfo::with_icon("html", "HTML", ""));
        self.register_extension("css", FiletypeInfo::with_icon("css", "CSS", ""));
        self.register_extension("scss", FiletypeInfo::with_icon("scss", "SCSS", ""));
        self.register_extension("sass", FiletypeInfo::with_icon("sass", "Sass", ""));

        // Database
        self.register_extension("sql", FiletypeInfo::with_icon("sql", "SQL", ""));

        // Vim
        self.register_extension("vim", FiletypeInfo::with_icon("vim", "Vim", ""));

        // Text
        self.register_extension("txt", FiletypeInfo::new("text", "Text"));

        // Java/Kotlin
        self.register_extension("java", FiletypeInfo::with_icon("java", "Java", ""));
        self.register_extension("kt", FiletypeInfo::with_icon("kotlin", "Kotlin", ""));
        self.register_extension("kts", FiletypeInfo::with_icon("kotlin", "Kotlin", ""));

        // Ruby
        self.register_extension("rb", FiletypeInfo::with_icon("ruby", "Ruby", ""));
        self.register_extension("erb", FiletypeInfo::with_icon("eruby", "ERB", ""));

        // PHP
        self.register_extension("php", FiletypeInfo::with_icon("php", "PHP", ""));

        // Swift
        self.register_extension("swift", FiletypeInfo::with_icon("swift", "Swift", ""));

        // Zig
        self.register_extension("zig", FiletypeInfo::with_icon("zig", "Zig", ""));

        // Elixir
        self.register_extension("ex", FiletypeInfo::with_icon("elixir", "Elixir", ""));
        self.register_extension("exs", FiletypeInfo::with_icon("elixir", "Elixir", ""));

        // Special filenames
        self.register_filename("Makefile", FiletypeInfo::with_icon("make", "Makefile", ""));
        self.register_filename("makefile", FiletypeInfo::with_icon("make", "Makefile", ""));
        self.register_filename("GNUmakefile", FiletypeInfo::with_icon("make", "Makefile", ""));
        self.register_filename(
            "Dockerfile",
            FiletypeInfo::with_icon("dockerfile", "Dockerfile", ""),
        );
        self.register_filename(
            "Containerfile",
            FiletypeInfo::with_icon("dockerfile", "Dockerfile", ""),
        );
        self.register_filename(
            ".gitignore",
            FiletypeInfo::with_icon("gitignore", "Git Ignore", ""),
        );
        self.register_filename(
            ".gitattributes",
            FiletypeInfo::new("gitattributes", "Git Attributes"),
        );
        self.register_filename("Cargo.toml", FiletypeInfo::with_icon("toml", "Cargo.toml", ""));
        self.register_filename("Cargo.lock", FiletypeInfo::with_icon("toml", "Cargo.lock", ""));
        self.register_filename("package.json", FiletypeInfo::with_icon("json", "package.json", ""));
        self.register_filename(
            "tsconfig.json",
            FiletypeInfo::with_icon("json", "tsconfig.json", ""),
        );
    }

    /// Register a file extension mapping.
    ///
    /// Extension should be lowercase without the leading dot.
    pub fn register_extension(&mut self, ext: &'static str, info: FiletypeInfo) {
        self.extension_map.insert(ext, info);
    }

    /// Register an exact filename mapping.
    pub fn register_filename(&mut self, name: &'static str, info: FiletypeInfo) {
        self.filename_map.insert(name, info);
    }

    /// Detect filetype from a file path.
    ///
    /// Detection order:
    /// 1. Exact filename match (e.g., "Makefile")
    /// 2. Extension match (case-insensitive)
    ///
    /// # Arguments
    ///
    /// * `path` - File path (can be absolute or relative)
    ///
    /// # Returns
    ///
    /// `Some(&FiletypeInfo)` if filetype is recognized, `None` otherwise.
    #[must_use]
    pub fn detect(&self, path: &str) -> Option<&FiletypeInfo> {
        // Extract filename from path
        let filename = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path);

        // First try exact filename match
        if let Some(info) = self.filename_map.get(filename) {
            return Some(info);
        }

        // Then try extension match (case-insensitive)
        let ext = Path::new(filename).extension()?.to_str()?;
        let ext_lower = ext.to_lowercase();
        self.extension_map.get(ext_lower.as_str())
    }

    /// Get filetype ID from path.
    ///
    /// Returns an empty string if filetype is not recognized.
    #[must_use]
    pub fn filetype_id(&self, path: &str) -> &'static str {
        self.detect(path).map_or("", |info| info.id)
    }

    /// Get display name from path.
    ///
    /// Returns an empty string if filetype is not recognized.
    #[must_use]
    pub fn display_name(&self, path: &str) -> &'static str {
        self.detect(path).map_or("", |info| info.display_name)
    }

    /// Get icon from path.
    ///
    /// Returns `None` if filetype is not recognized or has no icon.
    #[must_use]
    pub fn icon(&self, path: &str) -> Option<&'static str> {
        self.detect(path).and_then(|info| info.icon)
    }

    /// Check if a filetype is registered for the given extension.
    #[must_use]
    pub fn has_extension(&self, ext: &str) -> bool {
        self.extension_map.contains_key(ext)
    }

    /// Check if a filetype is registered for the given filename.
    #[must_use]
    pub fn has_filename(&self, name: &str) -> bool {
        self.filename_map.contains_key(name)
    }

    /// Get the number of registered extensions.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // HashMap::len() is not const
    pub fn extension_count(&self) -> usize {
        self.extension_map.len()
    }

    /// Get the number of registered filenames.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // HashMap::len() is not const
    pub fn filename_count(&self) -> usize {
        self.filename_map.len()
    }
}

/// Global filetype registry instance.
static GLOBAL_REGISTRY: OnceLock<FiletypeRegistry> = OnceLock::new();

/// Get the global filetype registry.
///
/// The registry is lazily initialized on first access.
#[must_use]
pub fn global_registry() -> &'static FiletypeRegistry {
    GLOBAL_REGISTRY.get_or_init(FiletypeRegistry::new)
}

/// Detect filetype from path using the global registry.
///
/// Convenience function for quick filetype detection.
///
/// # Example
///
/// ```
/// use reovim_driver_vfs::detect_filetype;
///
/// if let Some(info) = detect_filetype("main.rs") {
///     println!("File type: {}", info.display_name);
/// }
/// ```
#[must_use]
pub fn detect_filetype(path: &str) -> Option<&'static FiletypeInfo> {
    global_registry().detect(path)
}

/// Get filetype ID from path using the global registry.
///
/// Returns an empty string if filetype is not recognized.
#[must_use]
pub fn filetype_id(path: &str) -> &'static str {
    global_registry().filetype_id(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_by_extension() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("main.rs"), "rust");
        assert_eq!(registry.filetype_id("script.py"), "python");
        assert_eq!(registry.filetype_id("app.js"), "javascript");
        assert_eq!(registry.filetype_id("config.toml"), "toml");
    }

    #[test]
    fn test_detect_by_filename() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("Makefile"), "make");
        assert_eq!(registry.filetype_id("Dockerfile"), "dockerfile");
        assert_eq!(registry.filetype_id(".gitignore"), "gitignore");
    }

    #[test]
    fn test_detect_with_path() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("/home/user/project/src/main.rs"), "rust");
        assert_eq!(registry.filetype_id("./scripts/build.sh"), "bash");
    }

    #[test]
    fn test_unknown_extension() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("file.unknown"), "");
        assert_eq!(registry.filetype_id("no_extension"), "");
    }

    #[test]
    fn test_case_insensitive_extension() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("main.RS"), "rust");
        assert_eq!(registry.filetype_id("script.PY"), "python");
        assert_eq!(registry.filetype_id("config.TOML"), "toml");
    }

    #[test]
    fn test_filetype_info() {
        let registry = FiletypeRegistry::new();

        let info = registry.detect("main.rs").unwrap();
        assert_eq!(info.id, "rust");
        assert_eq!(info.display_name, "Rust");
        assert_eq!(info.icon, Some(""));
    }

    #[test]
    fn test_global_registry() {
        assert_eq!(filetype_id("test.rs"), "rust");
        assert!(detect_filetype("test.py").is_some());
    }

    #[test]
    fn test_empty_registry() {
        let registry = FiletypeRegistry::empty();

        assert_eq!(registry.extension_count(), 0);
        assert_eq!(registry.filename_count(), 0);
        assert!(registry.detect("main.rs").is_none());
    }

    #[test]
    fn test_custom_registration() {
        let mut registry = FiletypeRegistry::empty();

        registry.register_extension("custom", FiletypeInfo::new("custom", "Custom"));
        registry.register_filename("CustomFile", FiletypeInfo::new("customfile", "Custom File"));

        assert_eq!(registry.filetype_id("test.custom"), "custom");
        assert_eq!(registry.filetype_id("CustomFile"), "customfile");
    }

    #[test]
    fn test_icon_detection() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.icon("main.rs"), Some(""));
        assert_eq!(registry.icon("file.txt"), None);
        assert_eq!(registry.icon("unknown.xyz"), None);
    }

    #[test]
    fn test_display_name() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.display_name("main.rs"), "Rust");
        assert_eq!(registry.display_name("script.py"), "Python");
        assert_eq!(registry.display_name("unknown.xyz"), "");
    }

    #[test]
    fn test_has_methods() {
        let registry = FiletypeRegistry::new();

        assert!(registry.has_extension("rs"));
        assert!(registry.has_extension("py"));
        assert!(!registry.has_extension("unknown"));

        assert!(registry.has_filename("Makefile"));
        assert!(registry.has_filename("Dockerfile"));
        assert!(!registry.has_filename("RandomFile"));
    }

    #[test]
    fn test_filetype_registry_default() {
        let registry = FiletypeRegistry::default();
        // Default should behave same as new()
        assert!(registry.has_extension("rs"));
        assert!(registry.extension_count() > 0);
    }

    #[test]
    fn test_detect_cpp_extensions() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("main.cpp"), "cpp");
        assert_eq!(registry.filetype_id("main.cc"), "cpp");
        assert_eq!(registry.filetype_id("main.cxx"), "cpp");
        assert_eq!(registry.filetype_id("header.hpp"), "cpp");
        assert_eq!(registry.filetype_id("header.hxx"), "cpp");
    }

    #[test]
    fn test_detect_c_extensions() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("main.c"), "c");
        assert_eq!(registry.filetype_id("header.h"), "c");
    }

    #[test]
    fn test_detect_python_extensions() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("script.py"), "python");
        assert_eq!(registry.filetype_id("stub.pyi"), "python");
        assert_eq!(registry.filetype_id("script.pyw"), "python");
    }

    #[test]
    fn test_detect_javascript_typescript_extensions() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("app.js"), "javascript");
        assert_eq!(registry.filetype_id("module.mjs"), "javascript");
        assert_eq!(registry.filetype_id("module.cjs"), "javascript");
        assert_eq!(registry.filetype_id("app.ts"), "typescript");
        assert_eq!(registry.filetype_id("component.jsx"), "javascriptreact");
        assert_eq!(registry.filetype_id("component.tsx"), "typescriptreact");
    }

    #[test]
    fn test_detect_go() {
        let registry = FiletypeRegistry::new();
        assert_eq!(registry.filetype_id("main.go"), "go");
    }

    #[test]
    fn test_detect_lua() {
        let registry = FiletypeRegistry::new();
        assert_eq!(registry.filetype_id("init.lua"), "lua");
    }

    #[test]
    fn test_detect_shell_extensions() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("script.sh"), "bash");
        assert_eq!(registry.filetype_id("script.bash"), "bash");
        assert_eq!(registry.filetype_id("script.zsh"), "zsh");
        assert_eq!(registry.filetype_id("script.fish"), "fish");
    }

    #[test]
    fn test_detect_data_formats() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("config.json"), "json");
        assert_eq!(registry.filetype_id("config.toml"), "toml");
        assert_eq!(registry.filetype_id("config.yaml"), "yaml");
        assert_eq!(registry.filetype_id("config.yml"), "yaml");
        assert_eq!(registry.filetype_id("data.xml"), "xml");
    }

    #[test]
    fn test_detect_markup_extensions() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("readme.md"), "markdown");
        assert_eq!(registry.filetype_id("doc.markdown"), "markdown");
        assert_eq!(registry.filetype_id("page.html"), "html");
        assert_eq!(registry.filetype_id("page.htm"), "html");
        assert_eq!(registry.filetype_id("style.css"), "css");
        assert_eq!(registry.filetype_id("style.scss"), "scss");
        assert_eq!(registry.filetype_id("style.sass"), "sass");
    }

    #[test]
    fn test_detect_database_extensions() {
        let registry = FiletypeRegistry::new();
        assert_eq!(registry.filetype_id("query.sql"), "sql");
    }

    #[test]
    fn test_detect_vim_extension() {
        let registry = FiletypeRegistry::new();
        assert_eq!(registry.filetype_id("init.vim"), "vim");
    }

    #[test]
    fn test_detect_text_extension() {
        let registry = FiletypeRegistry::new();
        let info = registry.detect("notes.txt").unwrap();
        assert_eq!(info.id, "text");
        assert_eq!(info.display_name, "Text");
        assert_eq!(info.icon, None); // Text has no icon
    }

    #[test]
    fn test_detect_java_kotlin() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("Main.java"), "java");
        assert_eq!(registry.filetype_id("App.kt"), "kotlin");
        assert_eq!(registry.filetype_id("build.kts"), "kotlin");
    }

    #[test]
    fn test_detect_ruby_extensions() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("app.rb"), "ruby");
        assert_eq!(registry.filetype_id("template.erb"), "eruby");
    }

    #[test]
    fn test_detect_php() {
        let registry = FiletypeRegistry::new();
        assert_eq!(registry.filetype_id("index.php"), "php");
    }

    #[test]
    fn test_detect_swift() {
        let registry = FiletypeRegistry::new();
        assert_eq!(registry.filetype_id("App.swift"), "swift");
    }

    #[test]
    fn test_detect_zig() {
        let registry = FiletypeRegistry::new();
        assert_eq!(registry.filetype_id("main.zig"), "zig");
    }

    #[test]
    fn test_detect_elixir() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("app.ex"), "elixir");
        assert_eq!(registry.filetype_id("test.exs"), "elixir");
    }

    #[test]
    fn test_detect_special_filenames() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.filetype_id("makefile"), "make");
        assert_eq!(registry.filetype_id("GNUmakefile"), "make");
        assert_eq!(registry.filetype_id("Containerfile"), "dockerfile");
        assert_eq!(registry.filetype_id(".gitattributes"), "gitattributes");
        assert_eq!(registry.filetype_id("Cargo.toml"), "toml");
        assert_eq!(registry.filetype_id("Cargo.lock"), "toml");
        assert_eq!(registry.filetype_id("package.json"), "json");
        assert_eq!(registry.filetype_id("tsconfig.json"), "json");
    }

    #[test]
    fn test_filetype_info_new() {
        let info = FiletypeInfo::new("test", "Test Lang");
        assert_eq!(info.id, "test");
        assert_eq!(info.display_name, "Test Lang");
        assert_eq!(info.icon, None);
    }

    #[test]
    fn test_filetype_info_with_icon() {
        let info = FiletypeInfo::with_icon("test", "Test Lang", "X");
        assert_eq!(info.id, "test");
        assert_eq!(info.display_name, "Test Lang");
        assert_eq!(info.icon, Some("X"));
    }

    #[test]
    fn test_filetype_info_equality() {
        let a = FiletypeInfo::new("rust", "Rust");
        let b = FiletypeInfo::new("rust", "Rust");
        let c = FiletypeInfo::new("python", "Python");

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_filetype_info_clone() {
        let original = FiletypeInfo::with_icon("rust", "Rust", "R");
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_registry_extension_count() {
        let registry = FiletypeRegistry::new();
        // Should have many extensions registered
        assert!(registry.extension_count() > 30);
    }

    #[test]
    fn test_registry_filename_count() {
        let registry = FiletypeRegistry::new();
        // Should have several filenames registered
        assert!(registry.filename_count() >= 10);
    }

    #[test]
    fn test_custom_registration_overrides() {
        let mut registry = FiletypeRegistry::new();

        // Override the existing "rs" extension
        registry.register_extension("rs", FiletypeInfo::new("custom_rust", "Custom Rust"));
        assert_eq!(registry.filetype_id("test.rs"), "custom_rust");
    }

    #[test]
    fn test_detect_with_directory_path() {
        let registry = FiletypeRegistry::new();
        // Filename match in a nested path
        assert_eq!(registry.filetype_id("/home/user/project/Dockerfile"), "dockerfile");
    }

    #[test]
    fn test_detect_filename_priority_over_extension() {
        let registry = FiletypeRegistry::new();
        // "Cargo.toml" should match filename first, not just ".toml" extension
        let info = registry.detect("Cargo.toml").unwrap();
        assert_eq!(info.display_name, "Cargo.toml"); // Filename match gives "Cargo.toml"
    }

    #[test]
    fn test_global_registry_detect_filetype() {
        // Test the convenience functions
        let info = detect_filetype("main.rs");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.id, "rust");
    }

    #[test]
    fn test_global_registry_unknown() {
        assert!(detect_filetype("file.xyzabc").is_none());
        assert_eq!(filetype_id("file.xyzabc"), "");
    }

    #[test]
    fn test_display_name_for_known_types() {
        let registry = FiletypeRegistry::new();

        assert_eq!(registry.display_name("main.go"), "Go");
        assert_eq!(registry.display_name("init.lua"), "Lua");
        assert_eq!(registry.display_name("query.sql"), "SQL");
    }

    #[test]
    fn test_icon_for_various_types() {
        let registry = FiletypeRegistry::new();

        // Types with icons
        assert!(registry.icon("main.rs").is_some());
        assert!(registry.icon("app.py").is_some());
        assert!(registry.icon("main.go").is_some());

        // Text has no icon
        assert!(registry.icon("notes.txt").is_none());
    }
}
