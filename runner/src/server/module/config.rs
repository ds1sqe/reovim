//! Module configuration for runtime loading.
//!
//! Provides configuration for module discovery and auto-loading.
//! Configuration is read from `~/.config/reovim/config.toml` (or platform equivalent).
//!
//! # Config File Format
//!
//! ```toml
//! [modules]
//! search_paths = ["~/.local/share/reovim/modules"]
//! autoload = ["lang-rust", "feat-completion"]
//! ```
//!
//! # Platform Behavior
//!
//! - **Linux**: `~/.config/reovim/config.toml`
//! - **macOS**: `~/Library/Application Support/reovim/config.toml`
//! - **Windows**: `%APPDATA%\reovim\config.toml`

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::loading::default_search_paths;

/// Module configuration section from config file.
///
/// # Example
///
/// ```toml
/// [modules]
/// search_paths = ["~/.local/share/reovim/modules", "/usr/lib/reovim/modules"]
/// autoload = ["lang-rust", "feat-completion"]
/// no_defaults = false
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModuleConfig {
    /// Additional search paths for modules.
    ///
    /// Paths support tilde expansion (`~` → home directory).
    /// These are searched after the default paths.
    #[serde(default)]
    pub search_paths: Vec<String>,

    /// Module IDs to auto-load on startup.
    ///
    /// Modules are loaded in the order specified. Dependencies are
    /// resolved automatically via topological sort.
    #[serde(default)]
    pub autoload: Vec<String>,

    /// Skip loading default modules.
    ///
    /// When `true`, only modules specified in `autoload` (or via CLI `--load`)
    /// are loaded. Useful for testing or minimal startup.
    #[serde(default)]
    pub no_defaults: bool,
}

/// Top-level configuration file structure.
///
/// Only the `[modules]` section is relevant for module loading.
/// Other sections may be added for editor configuration.
#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigFile {
    /// Module configuration section.
    #[serde(default)]
    modules: ModuleConfig,
}

/// Error type for configuration loading.
#[derive(Debug)]
pub enum ConfigError {
    /// IO error reading config file.
    Io(io::Error),
    /// TOML parsing error.
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "config IO error: {e}"),
            Self::Parse(e) => write!(f, "config parse error: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

impl ModuleConfig {
    /// Create an empty module configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load module configuration from the default config file.
    ///
    /// Returns default configuration if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load() -> Result<Self, ConfigError> {
        let Some(path) = config_file_path() else {
            return Ok(Self::default());
        };

        Self::load_from(&path)
    }

    /// Load module configuration from a specific file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML config file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let config: ConfigFile = toml::from_str(&content)?;

        Ok(config.modules)
    }

    /// Get all search paths (defaults + configured).
    ///
    /// Returns default search paths followed by any additional paths
    /// from configuration. Tilde expansion is performed on configured paths.
    #[must_use]
    pub fn all_search_paths(&self) -> Vec<PathBuf> {
        let mut paths = default_search_paths();

        for path_str in &self.search_paths {
            if let Some(expanded) = expand_tilde(path_str) {
                paths.push(expanded);
            }
        }

        paths
    }

    /// Get the list of modules to auto-load.
    #[must_use]
    pub fn autoload_modules(&self) -> &[String] {
        &self.autoload
    }

    /// Check if any modules should be auto-loaded.
    #[must_use]
    pub const fn has_autoload(&self) -> bool {
        !self.autoload.is_empty()
    }

    /// Builder: add a search path.
    #[must_use]
    pub fn with_search_path(mut self, path: impl Into<String>) -> Self {
        self.search_paths.push(path.into());
        self
    }

    /// Builder: add a module to auto-load.
    #[must_use]
    pub fn with_autoload(mut self, module_id: impl Into<String>) -> Self {
        self.autoload.push(module_id.into());
        self
    }

    /// Builder: skip loading default modules.
    #[must_use]
    pub const fn with_no_defaults(mut self) -> Self {
        self.no_defaults = true;
        self
    }

    /// Check if default modules should be skipped.
    #[must_use]
    pub const fn should_skip_defaults(&self) -> bool {
        self.no_defaults
    }
}

/// Get the path to the config file.
///
/// Returns `~/.config/reovim/config.toml` on Linux (or platform equivalent).
fn config_file_path() -> Option<PathBuf> {
    reovim_arch::dirs::config_dir().map(|d| d.join("reovim").join("config.toml"))
}

/// Expand tilde (~) in a path string to the home directory.
///
/// # Examples
///
/// - `~/.local/share` → `/home/user/.local/share`
/// - `/absolute/path` → `/absolute/path` (unchanged)
/// - `relative/path` → `relative/path` (unchanged)
fn expand_tilde(path: &str) -> Option<PathBuf> {
    path.strip_prefix("~/").map_or_else(
        || {
            if path == "~" {
                reovim_arch::dirs::home_dir()
            } else {
                Some(PathBuf::from(path))
            }
        },
        |stripped| reovim_arch::dirs::home_dir().map(|home| home.join(stripped)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_config_default() {
        let config = ModuleConfig::new();
        assert!(config.search_paths.is_empty());
        assert!(config.autoload.is_empty());
        assert!(!config.has_autoload());
    }

    #[test]
    fn test_module_config_builder() {
        let config = ModuleConfig::new()
            .with_search_path("/custom/path")
            .with_autoload("my-module");

        assert_eq!(config.search_paths, vec!["/custom/path"]);
        assert_eq!(config.autoload, vec!["my-module"]);
        assert!(config.has_autoload());
    }

    #[test]
    fn test_all_search_paths_includes_defaults() {
        let config = ModuleConfig::new();
        let paths = config.all_search_paths();

        // Should include default paths
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_all_search_paths_includes_custom() {
        let config = ModuleConfig::new().with_search_path("/custom/modules");

        let paths = config.all_search_paths();
        assert!(paths.contains(&PathBuf::from("/custom/modules")));
    }

    #[test]
    fn test_expand_tilde_with_home() {
        let expanded = expand_tilde("~/.local/share");
        assert!(expanded.is_some());

        let path = expanded.unwrap();
        // Should not contain tilde
        assert!(!path.to_string_lossy().contains('~'));
        // Should end with .local/share
        assert!(path.ends_with(".local/share"));
    }

    #[test]
    fn test_expand_tilde_absolute_path() {
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, Some(PathBuf::from("/absolute/path")));
    }

    #[test]
    fn test_expand_tilde_relative_path() {
        let expanded = expand_tilde("relative/path");
        assert_eq!(expanded, Some(PathBuf::from("relative/path")));
    }

    #[test]
    fn test_expand_tilde_just_tilde() {
        let expanded = expand_tilde("~");
        assert!(expanded.is_some());
        assert_eq!(expanded, reovim_arch::dirs::home_dir());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = ModuleConfig::load_from(Path::new("/nonexistent/config.toml"));
        // Should return default config, not error
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.autoload.is_empty());
    }

    #[test]
    fn test_parse_toml() {
        let toml_content = r#"
[modules]
search_paths = ["/usr/lib/reovim/modules", "~/.local/share/reovim/modules"]
autoload = ["lang-rust", "feat-lsp"]
"#;

        let config: ConfigFile = toml::from_str(toml_content).unwrap();
        assert_eq!(config.modules.search_paths.len(), 2);
        assert_eq!(config.modules.autoload.len(), 2);
        assert_eq!(config.modules.autoload[0], "lang-rust");
        assert_eq!(config.modules.autoload[1], "feat-lsp");
    }

    #[test]
    fn test_parse_empty_toml() {
        let toml_content = "";

        let config: ConfigFile = toml::from_str(toml_content).unwrap();
        assert!(config.modules.search_paths.is_empty());
        assert!(config.modules.autoload.is_empty());
    }

    #[test]
    fn test_parse_no_modules_section() {
        let toml_content = r#"
[editor]
theme = "dark"
"#;

        let config: ConfigFile = toml::from_str(toml_content).unwrap();
        assert!(config.modules.search_paths.is_empty());
        assert!(config.modules.autoload.is_empty());
    }

    #[test]
    fn test_config_error_display() {
        let io_error = ConfigError::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        assert!(io_error.to_string().contains("config IO error"));

        // Parse error is harder to construct, but we can verify the Display impl exists
        let _ = format!("{io_error}");
    }

    #[test]
    fn test_autoload_modules() {
        let config = ModuleConfig::new()
            .with_autoload("mod-a")
            .with_autoload("mod-b");

        let autoload = config.autoload_modules();
        assert_eq!(autoload.len(), 2);
        assert_eq!(autoload[0], "mod-a");
        assert_eq!(autoload[1], "mod-b");
    }

    #[test]
    fn test_no_defaults_default() {
        let config = ModuleConfig::new();
        assert!(!config.no_defaults);
        assert!(!config.should_skip_defaults());
    }

    #[test]
    fn test_no_defaults_builder() {
        let config = ModuleConfig::new().with_no_defaults();
        assert!(config.no_defaults);
        assert!(config.should_skip_defaults());
    }

    #[test]
    fn test_parse_toml_with_no_defaults() {
        let toml_content = r#"
[modules]
autoload = ["my-module"]
no_defaults = true
"#;

        let config: ConfigFile = toml::from_str(toml_content).unwrap();
        assert!(config.modules.no_defaults);
        assert_eq!(config.modules.autoload, vec!["my-module"]);
    }

    #[test]
    fn test_parse_toml_no_defaults_false() {
        let toml_content = r"
[modules]
no_defaults = false
";

        let config: ConfigFile = toml::from_str(toml_content).unwrap();
        assert!(!config.modules.no_defaults);
    }
}
