//! Theme file discovery and loading.
//!
//! `ThemeLoader` searches for theme files in standard locations and
//! delegates parsing to the registered [`ThemeFactory`]. File I/O is
//! mechanism (not policy), so it lives in this server-tier crate; TOML
//! parsing and `Style` resolution stay on the display side via the
//! factory hook.
//!
//! # Search Paths
//!
//! Themes are searched in order:
//! 1. `$REOVIM_THEME_DIR` (env override, highest priority)
//! 2. `~/.config/reovim/themes/` (user themes)
//! 3. `~/.local/share/reovim/themes/` (XDG data dir)
//! 4. `/usr/share/reovim/themes/` (Unix system themes)
//! 5. `/usr/local/share/reovim/themes/` (Unix local install)
//!
//! [`ThemeFactory`]: super::ThemeFactory

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{BuiltinTheme, ThemeError, factory::theme_factory, provider::ThemeProvider};

/// Information about a discovered theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeInfo {
    /// Theme filename (without `.toml` extension).
    pub name: String,
    /// Full path to the theme file. `None` for built-in themes.
    pub path: Option<PathBuf>,
    /// Whether this is a built-in theme.
    pub builtin: bool,
}

/// Discovers and loads theme files from standard locations.
pub struct ThemeLoader {
    /// Ordered list of directories to search for themes.
    search_paths: Vec<PathBuf>,
}

impl ThemeLoader {
    /// Create a new theme loader with default search paths.
    ///
    /// See the module docs for the resolution order.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new() -> Self {
        let mut search_paths = Vec::new();

        if let Ok(env_dir) = std::env::var("REOVIM_THEME_DIR") {
            search_paths.push(PathBuf::from(env_dir));
        }

        if let Some(config_dir) = dirs::config_dir() {
            search_paths.push(config_dir.join("reovim").join("themes"));
        }

        if let Some(data_dir) = dirs::data_dir() {
            search_paths.push(data_dir.join("reovim").join("themes"));
        }

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
    /// Paths are searched in order, with earlier paths taking
    /// priority.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
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
    /// - Just the theme name (e.g., `"tokyo-night"`) — searches all paths.
    /// - An absolute path to a theme file.
    /// - A relative path to a theme file.
    ///
    /// Theme files should have `.toml` extension. If a file is found,
    /// the registered [`ThemeFactory`] parses its content; if no
    /// factory is registered the call falls back to the built-in stub
    /// for matching variants (sufficient for server-only deployments).
    ///
    /// # Errors
    ///
    /// Returns [`ThemeError`] if:
    /// - the file cannot be read,
    /// - parsing fails (delegated to the factory),
    /// - the name matches neither a file nor a built-in.
    ///
    /// [`ThemeFactory`]: super::ThemeFactory
    pub fn load(&self, name: &str) -> Result<Arc<dyn ThemeProvider>, ThemeError> {
        self.resolve_path(name)
            .map_or_else(|_| Self::builtin_fallback(name), |path| Self::load_from_path(&path, name))
    }

    /// Read a resolved theme file and dispatch parsing to the
    /// registered [`ThemeFactory`].
    ///
    /// Coverage carve-out: the no-factory branch is exercised only in
    /// display-tier integration tests (where the display crate
    /// installs its `DisplayThemeFactory`); the registry crate's own
    /// test binary cannot deterministically toggle the process-global
    /// factory between tests, so this helper is marked
    /// `coverage(off)`. The successful factory path returns whatever
    /// the factory produces, and the no-factory branch returns a
    /// stable `Unsupported` error.
    ///
    /// [`ThemeFactory`]: super::ThemeFactory
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn load_from_path(path: &Path, name: &str) -> Result<Arc<dyn ThemeProvider>, ThemeError> {
        let content = std::fs::read_to_string(path)?;
        theme_factory().map_or_else(
            || {
                Err(ThemeError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!("No theme factory registered: cannot parse file theme '{name}'"),
                )))
            },
            |factory| factory.load_file(name, &content),
        )
    }

    /// Resolve a theme name to a built-in provider, or return
    /// `NotFound`.
    fn builtin_fallback(name: &str) -> Result<Arc<dyn ThemeProvider>, ThemeError> {
        for variant in BuiltinTheme::all() {
            if variant.name() == name {
                return Ok(variant.load());
            }
        }
        Err(ThemeError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Theme '{name}' not found"),
        )))
    }

    /// List all available theme names.
    ///
    /// Returns unique theme names found in all search paths. Names
    /// are returned without the `.toml` extension.
    ///
    /// # Panics
    ///
    /// Panics if a `.toml` directory entry has no file stem
    /// (structurally impossible for valid filesystem entries).
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
    /// File themes shadow built-in themes with the same name.
    /// Results are sorted alphabetically by name.
    ///
    /// # Panics
    ///
    /// Panics if a `.toml` directory entry has no file stem
    /// (structurally impossible for valid filesystem entries).
    #[must_use]
    pub fn discover(&self) -> Vec<ThemeInfo> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

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
        let as_path = Path::new(name);
        if as_path.is_absolute() && as_path.exists() {
            return Some(as_path.to_path_buf());
        }

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
        let as_path = Path::new(name);
        if as_path.exists() {
            return Ok(as_path.to_path_buf());
        }

        self.find_theme_path(name).ok_or_else(|| {
            ThemeError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Theme '{name}' not found in search paths"),
            ))
        })
    }

    /// Get the user themes directory (without creating it).
    ///
    /// Returns `None` if the config directory cannot be determined.
    #[must_use]
    pub fn user_themes_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("reovim").join("themes"))
    }

    /// Ensure the user themes directory exists, creating it if
    /// necessary.
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn default() -> Self {
        Self::new()
    }
}

impl reovim_kernel::api::v1::Service for ThemeLoader {}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
