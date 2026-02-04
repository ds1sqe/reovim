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
    ThemeProvider,
    file::{FileTheme, ThemeError},
};

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
    pub fn new() -> Self {
        let mut search_paths = Vec::new();

        // User config directory (highest priority)
        if let Some(config_dir) = dirs::config_dir() {
            search_paths.push(config_dir.join("reovim").join("themes"));
        }

        // System themes directory (platform-specific)
        #[cfg(unix)]
        {
            search_paths.push(PathBuf::from("/usr/share/reovim/themes"));
            search_paths.push(PathBuf::from("/usr/local/share/reovim/themes"));
        }

        #[cfg(windows)]
        {
            if let Some(data_dir) = dirs::data_dir() {
                search_paths.push(data_dir.join("reovim").join("themes"));
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
        let path = self.resolve_path(name)?;
        let content = std::fs::read_to_string(&path)?;
        let theme = FileTheme::parse(&content)?;
        Ok(theme.into_arc())
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
    #[must_use]
    pub fn list_available(&self) -> Vec<String> {
        let mut themes = HashSet::new();

        for path in &self.search_paths {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
                        && let Some(stem) = file_path.file_stem()
                    {
                        themes.insert(stem.to_string_lossy().into_owned());
                    }
                }
            }
        }

        let mut result: Vec<_> = themes.into_iter().collect();
        result.sort();
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
        let filename = if Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
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
}
