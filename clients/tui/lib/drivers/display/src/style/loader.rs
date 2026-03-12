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
#[path = "loader_tests.rs"]
mod tests;
