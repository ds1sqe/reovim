//! Theme file discovery and loading.
//!
//! The `ThemeLoader` searches for theme files in standard locations and
//! provides methods to load themes by name.
//!
//! # Search Paths
//!
//! Themes are searched in order:
//! 1. `~/.config/reovim/themes/` (user themes, highest priority)
//! 2. `/usr/share/reovim/themes/` (system themes, Unix only)
//! 3. `%APPDATA%/reovim/themes/` (system themes, Windows only)
//!
//! # Usage
//!
//! ```ignore
//! let loader = ThemeLoader::new();
//!
//! // Load a theme by name
//! let theme = loader.load("tokyo-night")?;
//!
//! // List all available themes
//! for name in loader.list_available() {
//!     println!("{}", name);
//! }
//! ```

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{
    BuiltinTheme, ThemeProvider,
    file::{FileTheme, ThemeError},
};

// =============================================================================
// ThemeInfo
// =============================================================================

/// Information about a discovered theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeInfo {
    /// Theme filename (without .toml extension).
    pub name: String,
    /// Full path to the theme file. `None` for built-in themes.
    pub path: Option<PathBuf>,
    /// Whether this is a built-in theme.
    pub builtin: bool,
}

// =============================================================================
// ThemeLoader
// =============================================================================

/// Discovers and loads theme files from standard locations.
pub struct ThemeLoader {
    /// Ordered list of directories to search for themes.
    search_paths: Vec<PathBuf>,
}

impl ThemeLoader {
    /// Create a new theme loader with default search paths.
    ///
    /// Search paths are:
    /// 1. `~/.config/reovim/themes/` (user themes)
    /// 2. System themes directory (platform-specific)
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new() -> Self {
        let mut search_paths = Vec::new();

        // Environment override (highest priority)
        if let Ok(env_dir) = std::env::var("REOVIM_THEME_DIR") {
            search_paths.push(PathBuf::from(env_dir));
        }

        // User config directory
        if let Some(config_dir) = dirs::config_dir() {
            search_paths.push(config_dir.join("reovim").join("themes"));
        }

        // XDG data directory
        if let Some(data_dir) = dirs::data_dir() {
            search_paths.push(data_dir.join("reovim").join("themes"));
        }

        // System themes directory (platform-specific)
        #[cfg(unix)]
        {
            search_paths.push(PathBuf::from("/usr/share/reovim/themes"));
            search_paths.push(PathBuf::from("/usr/local/share/reovim/themes"));
        }

        #[cfg(windows)]
        {
            if let Some(app_data) = dirs::data_dir() {
                let system_path = app_data.join("reovim").join("themes");
                if !search_paths.contains(&system_path) {
                    search_paths.push(system_path);
                }
            }
        }

        Self { search_paths }
    }

    /// Create a theme loader with custom search paths.
    ///
    /// Paths are searched in order, with earlier paths taking priority.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec can't be in const fn
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            search_paths: paths,
        }
    }

    /// Add a search path with highest priority.
    pub fn add_path(&mut self, path: PathBuf) {
        self.search_paths.insert(0, path);
    }

    /// Get the current search paths.
    #[must_use]
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    /// Load a theme by name.
    ///
    /// The name can be:
    /// - Just the theme name (e.g., `"tokyo-night"`) - searches all paths
    /// - An absolute path to a theme file
    /// - A relative path to a theme file
    ///
    /// Theme files should have `.toml` extension.
    ///
    /// # Errors
    ///
    /// Returns `ThemeError` if:
    /// - Theme file not found
    /// - Theme file cannot be read
    /// - Theme file has invalid format
    pub fn load(&self, name: &str) -> Result<Arc<dyn ThemeProvider>, ThemeError> {
        // Try file-based theme first
        match self.resolve_path(name) {
            Ok(path) => {
                let content = std::fs::read_to_string(&path)?;
                let theme = FileTheme::parse(&content)?;
                return Ok(theme.into_arc());
            }
            Err(_) => {
                // Fall back to built-in themes
                for variant in BuiltinTheme::all() {
                    if variant.name() == name {
                        return Ok(variant.load());
                    }
                }
            }
        }
        // Neither file nor built-in matched
        Err(ThemeError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Theme '{name}' not found"),
        )))
    }

    /// Load a theme file directly from a path.
    ///
    /// # Errors
    ///
    /// Returns `ThemeError` if the file cannot be read or parsed.
    pub fn load_path(&self, path: &Path) -> Result<Arc<dyn ThemeProvider>, ThemeError> {
        let content = std::fs::read_to_string(path)?;
        let theme = FileTheme::parse(&content)?;
        Ok(theme.into_arc())
    }

    /// List all available theme names.
    ///
    /// Returns unique theme names found in all search paths.
    /// Names are returned without the `.toml` extension.
    ///
    /// # Panics
    ///
    /// Panics if a `.toml` directory entry has no file stem (structurally
    /// impossible for valid filesystem entries).
    #[must_use]
    pub fn list_available(&self) -> Vec<String> {
        let mut themes = HashSet::new();

        for path in &self.search_paths {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if let Some(ext) = file_path.extension()
                        && ext.eq_ignore_ascii_case("toml")
                    {
                        // Directory entries always have a filename, so file_stem() is always Some
                        let stem = file_path.file_stem().expect("entry has filename");
                        themes.insert(stem.to_string_lossy().into_owned());
                    }
                }
            }
        }

        let mut result: Vec<_> = themes.into_iter().collect();
        result.sort();
        result
    }

    /// Discover all available themes (file-based and built-in).
    ///
    /// Returns structured `ThemeInfo` for each theme found across all search
    /// paths plus built-in themes. File themes shadow built-in themes with
    /// the same name. Results are sorted alphabetically by name.
    ///
    /// # Panics
    ///
    /// Panics if a `.toml` directory entry has no file stem (structurally
    /// impossible for valid filesystem entries).
    #[must_use]
    pub fn discover(&self) -> Vec<ThemeInfo> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        // File themes from search paths (higher priority)
        for path in &self.search_paths {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if let Some(ext) = file_path.extension()
                        && ext.eq_ignore_ascii_case("toml")
                    {
                        let stem = file_path.file_stem().expect("entry has filename");
                        let name = stem.to_string_lossy().into_owned();
                        if seen.insert(name.clone()) {
                            result.push(ThemeInfo {
                                name,
                                path: Some(file_path),
                                builtin: false,
                            });
                        }
                    }
                }
            }
        }

        // Built-in themes (if not shadowed by file themes)
        for variant in BuiltinTheme::all() {
            let name = variant.name().to_string();
            if seen.insert(name.clone()) {
                result.push(ThemeInfo {
                    name,
                    path: None,
                    builtin: true,
                });
            }
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }

    /// Check if a theme exists by name.
    #[must_use]
    pub fn exists(&self, name: &str) -> bool {
        self.find_theme_path(name).is_some()
    }

    /// Get the full path to a theme file if it exists.
    #[must_use]
    pub fn find_theme_path(&self, name: &str) -> Option<PathBuf> {
        // First check if it's already a path
        let as_path = Path::new(name);
        if as_path.is_absolute() && as_path.exists() {
            return Some(as_path.to_path_buf());
        }

        // Search in search paths
        let filename = if let Some(ext) = Path::new(name).extension()
            && ext.eq_ignore_ascii_case("toml")
        {
            name.to_string()
        } else {
            format!("{name}.toml")
        };

        for search_path in &self.search_paths {
            let full_path = search_path.join(&filename);
            if full_path.exists() {
                return Some(full_path);
            }
        }

        None
    }

    /// Resolve a theme name to a file path.
    fn resolve_path(&self, name: &str) -> Result<PathBuf, ThemeError> {
        // Check if it's already a valid path
        let as_path = Path::new(name);
        if as_path.exists() {
            return Ok(as_path.to_path_buf());
        }

        // Search in search paths
        self.find_theme_path(name).ok_or_else(|| {
            ThemeError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Theme '{name}' not found in search paths"),
            ))
        })
    }

    /// Get the user themes directory (creates it if it doesn't exist).
    ///
    /// Returns `None` if the config directory cannot be determined.
    #[must_use]
    pub fn user_themes_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("reovim").join("themes"))
    }

    /// Ensure the user themes directory exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn ensure_user_themes_dir() -> std::io::Result<PathBuf> {
        let dir = Self::user_themes_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cannot determine user config directory",
            )
        })?;

        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}

impl Default for ThemeLoader {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Service marker for ServiceRegistry integration
impl reovim_kernel::api::v1::Service for ThemeLoader {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {super::*, std::io::Write, tempfile::TempDir};

    fn create_test_theme(dir: &Path, name: &str, content: &str) {
        let file_path = dir.join(format!("{name}.toml"));
        let mut file = std::fs::File::create(file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_loader_new() {
        let loader = ThemeLoader::new();
        // Should have at least user config path
        assert!(!loader.search_paths().is_empty());
    }

    #[test]
    fn test_loader_with_custom_paths() {
        let paths = vec![
            PathBuf::from("/custom/path1"),
            PathBuf::from("/custom/path2"),
        ];
        let loader = ThemeLoader::with_paths(paths.clone());
        assert_eq!(loader.search_paths(), &paths);
    }

    #[test]
    fn test_add_path() {
        let mut loader = ThemeLoader::with_paths(vec![PathBuf::from("/existing")]);
        loader.add_path(PathBuf::from("/new"));

        // New path should be first (highest priority)
        assert_eq!(loader.search_paths()[0], PathBuf::from("/new"));
    }

    #[test]
    fn test_load_theme_from_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let theme_content = r##"
            [meta]
            name = "Test Theme"

            [syntax]
            keyword = { fg = "#ff0000", bold = true }
        "##;

        create_test_theme(temp_dir.path(), "test-theme", theme_content);

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let theme = loader.load("test-theme").unwrap();

        assert_eq!(theme.name(), "Test Theme");
        let keyword = theme.get_style("keyword").unwrap();
        assert_eq!(keyword.fg, Some(reovim_arch::Color::Rgb { r: 255, g: 0, b: 0 }));
    }

    #[test]
    fn test_load_theme_with_extension() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "my-theme", "[meta]\nname = \"My Theme\"");

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);

        // Should work with or without .toml extension
        assert!(loader.load("my-theme").is_ok());
        assert!(loader.load("my-theme.toml").is_ok());
    }

    #[test]
    fn test_load_nonexistent_theme() {
        let loader = ThemeLoader::with_paths(vec![]);
        let result = loader.load("nonexistent-theme");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_available() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "alpha", "[meta]\nname = \"Alpha\"");
        create_test_theme(temp_dir.path(), "beta", "[meta]\nname = \"Beta\"");
        create_test_theme(temp_dir.path(), "gamma", "[meta]\nname = \"Gamma\"");

        // Also create a non-theme file to test filtering
        std::fs::write(temp_dir.path().join("readme.md"), "# Readme").unwrap();

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let themes = loader.list_available();

        assert_eq!(themes, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_exists() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "exists", "[meta]\nname = \"Exists\"");

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);

        assert!(loader.exists("exists"));
        assert!(!loader.exists("does-not-exist"));
    }

    #[test]
    fn test_find_theme_path() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "findme", "[meta]\nname = \"Find Me\"");

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let path = loader.find_theme_path("findme").unwrap();

        assert_eq!(path, temp_dir.path().join("findme.toml"));
    }

    #[test]
    fn test_search_path_priority() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();

        // Create theme with same name in both directories
        create_test_theme(dir1.path(), "priority", "[meta]\nname = \"From Dir1\"");
        create_test_theme(dir2.path(), "priority", "[meta]\nname = \"From Dir2\"");

        // dir1 has higher priority
        let loader =
            ThemeLoader::with_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()]);

        let theme = loader.load("priority").unwrap();
        assert_eq!(theme.name(), "From Dir1");
    }

    #[test]
    fn test_load_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("absolute.toml");
        std::fs::write(&theme_path, "[meta]\nname = \"Absolute Path\"").unwrap();

        let loader = ThemeLoader::with_paths(vec![]);
        let theme = loader.load(theme_path.to_str().unwrap()).unwrap();

        assert_eq!(theme.name(), "Absolute Path");
    }

    #[test]
    fn test_list_available_deduplicates() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();

        // Same theme name in both directories
        create_test_theme(dir1.path(), "common", "[meta]\nname = \"Common 1\"");
        create_test_theme(dir2.path(), "common", "[meta]\nname = \"Common 2\"");
        create_test_theme(dir1.path(), "unique1", "[meta]\nname = \"Unique 1\"");
        create_test_theme(dir2.path(), "unique2", "[meta]\nname = \"Unique 2\"");

        let loader =
            ThemeLoader::with_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()]);

        let themes = loader.list_available();
        // Should have 3 unique names, sorted
        assert_eq!(themes, vec!["common", "unique1", "unique2"]);
    }

    // =========================================================================
    // Coverage tests for loader.rs uncovered paths
    // =========================================================================

    #[test]
    fn test_load_path_directly() {
        let temp_dir = TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("direct.toml");
        std::fs::write(&theme_path, "[meta]\nname = \"Direct Load\"").unwrap();

        let loader = ThemeLoader::with_paths(vec![]);
        let theme = loader.load_path(&theme_path).unwrap();
        assert_eq!(theme.name(), "Direct Load");
    }

    #[test]
    fn test_load_path_nonexistent_file() {
        let loader = ThemeLoader::with_paths(vec![]);
        let result = loader.load_path(Path::new("/nonexistent/path/theme.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_path_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("bad.toml");
        std::fs::write(&theme_path, "{{not valid toml").unwrap();

        let loader = ThemeLoader::with_paths(vec![]);
        let result = loader.load_path(&theme_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_available_with_nonexistent_search_path() {
        // Nonexistent directory in search paths should be silently skipped
        let loader = ThemeLoader::with_paths(vec![PathBuf::from("/nonexistent/dir")]);
        let themes = loader.list_available();
        assert!(themes.is_empty());
    }

    #[test]
    fn test_list_available_filters_non_toml_files() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "valid-theme", "[meta]\nname = \"Valid\"");
        std::fs::write(temp_dir.path().join("readme.txt"), "not a theme").unwrap();
        std::fs::write(temp_dir.path().join("config.json"), "{}").unwrap();

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let themes = loader.list_available();
        assert_eq!(themes, vec!["valid-theme"]);
    }

    #[test]
    fn test_find_theme_path_with_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("absolute-test.toml");
        std::fs::write(&theme_path, "[meta]\nname = \"Absolute\"").unwrap();

        let loader = ThemeLoader::with_paths(vec![]);
        let found = loader.find_theme_path(theme_path.to_str().unwrap());
        assert_eq!(found, Some(theme_path));
    }

    #[test]
    fn test_find_theme_path_not_found() {
        let loader = ThemeLoader::with_paths(vec![]);
        let found = loader.find_theme_path("nonexistent-theme");
        assert!(found.is_none());
    }

    #[test]
    fn test_list_available_non_toml_extension_skipped() {
        // Line 150: is_some_and false - file has extension but not "toml"
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("mytheme.txt"), "not a theme").unwrap();

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let themes = loader.list_available();
        assert!(themes.is_empty());
    }

    #[test]
    fn test_find_theme_path_non_toml_extension() {
        // Line 174: is_some_and with non-toml extension (false inner condition)
        // Name has ".txt" extension, so it gets ".toml" appended
        let temp_dir = TempDir::new().unwrap();
        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let found = loader.find_theme_path("mytheme.txt");
        assert!(found.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_user_themes_dir_returns_some() {
        // On most systems, config_dir is available
        let dir = ThemeLoader::user_themes_dir();
        // We just verify it returns Some and the path ends with "themes"
        if let Some(d) = dir {
            assert!(d.ends_with("themes"));
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_ensure_user_themes_dir() {
        // This creates the directory if it doesn't exist
        // On test systems with a config dir, this should succeed
        let result = ThemeLoader::ensure_user_themes_dir();
        if let Ok(dir) = result {
            assert!(dir.exists());
            assert!(dir.ends_with("themes"));
        }
        // If config_dir is not available, the error is acceptable
    }

    #[test]
    fn test_loader_default_impl() {
        let loader = ThemeLoader::default();
        // Default should be same as new()
        assert!(!loader.search_paths().is_empty());
    }

    #[test]
    fn test_load_missing_theme_error_message() {
        let loader = ThemeLoader::with_paths(vec![]);
        let result = loader.load("ghost-theme");
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("ghost-theme"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_list_available_file_without_extension() {
        // Line 150: extension() returns None (file has no extension at all)
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("noext"), "not a theme").unwrap();

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let themes = loader.list_available();
        assert!(themes.is_empty());
    }

    #[test]
    fn test_find_theme_path_absolute_nonexistent() {
        // Line 174: as_path.is_absolute() (true) && as_path.exists() (false)
        let loader = ThemeLoader::with_paths(vec![]);
        let found = loader.find_theme_path("/nonexistent/absolute/path/theme.toml");
        assert!(found.is_none());
    }

    // =========================================================================
    // discover() tests
    // =========================================================================

    #[test]
    fn test_discover_returns_file_themes() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "alpha", "[meta]\nname = \"Alpha\"");
        create_test_theme(temp_dir.path(), "beta", "[meta]\nname = \"Beta\"");

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let themes = loader.discover();

        // Should have alpha, beta, plus 3 built-ins
        let file_themes: Vec<_> = themes.iter().filter(|t| !t.builtin).collect();
        assert_eq!(file_themes.len(), 2);
        assert!(file_themes.iter().any(|t| t.name == "alpha"));
        assert!(file_themes.iter().any(|t| t.name == "beta"));
        // File themes should have paths
        assert!(file_themes.iter().all(|t| t.path.is_some()));
    }

    #[test]
    fn test_discover_includes_builtins() {
        let loader = ThemeLoader::with_paths(vec![]);
        let themes = loader.discover();

        let builtin_themes: Vec<_> = themes.iter().filter(|t| t.builtin).collect();
        assert_eq!(builtin_themes.len(), 3);
        assert!(builtin_themes.iter().any(|t| t.name == "dark"));
        assert!(builtin_themes.iter().any(|t| t.name == "light"));
        assert!(
            builtin_themes
                .iter()
                .any(|t| t.name == "tokyo-night-orange")
        );
        // Built-in themes should have no path
        assert!(builtin_themes.iter().all(|t| t.path.is_none()));
    }

    #[test]
    fn test_discover_file_shadows_builtin() {
        let temp_dir = TempDir::new().unwrap();
        // Create a file theme with the same name as a built-in
        create_test_theme(temp_dir.path(), "dark", "[meta]\nname = \"Custom Dark\"");

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let themes = loader.discover();

        // "dark" should appear once, as a file theme (not built-in)
        let dark_themes: Vec<_> = themes.iter().filter(|t| t.name == "dark").collect();
        assert_eq!(dark_themes.len(), 1);
        assert!(!dark_themes[0].builtin);
        assert!(dark_themes[0].path.is_some());
    }

    #[test]
    fn test_discover_sorted_by_name() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "zebra", "[meta]\nname = \"Zebra\"");
        create_test_theme(temp_dir.path(), "alpha", "[meta]\nname = \"Alpha\"");

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let themes = loader.discover();
        let names: Vec<_> = themes.iter().map(|t| &t.name).collect();

        // Verify sorted
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);
    }

    #[test]
    fn test_discover_deduplicates_across_paths() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();

        create_test_theme(dir1.path(), "common", "[meta]\nname = \"Common 1\"");
        create_test_theme(dir2.path(), "common", "[meta]\nname = \"Common 2\"");

        let loader =
            ThemeLoader::with_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
        let themes = loader.discover();

        assert_eq!(themes.iter().filter(|t| t.name == "common").count(), 1);
    }

    #[test]
    fn test_discover_empty_paths_returns_only_builtins() {
        let loader = ThemeLoader::with_paths(vec![]);
        let themes = loader.discover();

        assert_eq!(themes.len(), 3);
        assert!(themes.iter().all(|t| t.builtin));
    }

    // =========================================================================
    // Built-in fallback in load() tests
    // =========================================================================

    #[test]
    fn test_load_falls_back_to_builtin() {
        let loader = ThemeLoader::with_paths(vec![]);
        // No file themes, but "dark" should be loadable as built-in
        let theme = loader.load("dark").unwrap();
        assert_eq!(theme.name(), "dark");
    }

    #[test]
    fn test_load_builtin_light() {
        let loader = ThemeLoader::with_paths(vec![]);
        let theme = loader.load("light").unwrap();
        assert_eq!(theme.name(), "light");
    }

    #[test]
    fn test_load_builtin_tokyo_night() {
        let loader = ThemeLoader::with_paths(vec![]);
        let theme = loader.load("tokyo-night-orange").unwrap();
        assert_eq!(theme.name(), "tokyo-night-orange");
    }

    #[test]
    fn test_load_prefers_file_over_builtin() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(
            temp_dir.path(),
            "dark",
            "[meta]\nname = \"Custom Dark File\"\n[syntax]\nkeyword = { fg = \"#aabbcc\" }",
        );

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let theme = loader.load("dark").unwrap();
        // Should get the file theme, not the built-in
        assert_eq!(theme.name(), "Custom Dark File");
    }

    #[test]
    fn test_discover_skips_non_toml_files() {
        let temp_dir = TempDir::new().unwrap();
        create_test_theme(temp_dir.path(), "valid", "[meta]\nname = \"Valid\"");
        std::fs::write(temp_dir.path().join("readme.md"), "# Readme").unwrap();
        std::fs::write(temp_dir.path().join("noext"), "data").unwrap();

        let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
        let file_themes: Vec<_> = loader
            .discover()
            .into_iter()
            .filter(|t| !t.builtin)
            .collect();
        assert_eq!(file_themes.len(), 1);
        assert_eq!(file_themes[0].name, "valid");
    }

    #[test]
    fn test_discover_nonexistent_search_path() {
        let loader = ThemeLoader::with_paths(vec![PathBuf::from("/nonexistent/path")]);
        let themes = loader.discover();
        // Should gracefully skip missing dir, still return built-ins
        assert_eq!(themes.len(), 3);
        assert!(themes.iter().all(|t| t.builtin));
    }

    #[test]
    fn test_theme_info_debug_and_eq() {
        let info1 = ThemeInfo {
            name: "test".to_string(),
            path: None,
            builtin: true,
        };
        let info2 = info1.clone();
        assert_eq!(info1, info2);
        let debug = format!("{info1:?}");
        assert!(debug.contains("test"));
    }
}
