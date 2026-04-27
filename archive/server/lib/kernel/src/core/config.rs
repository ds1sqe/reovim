//! Configuration mechanism for kernel.
//!
//! Linux equivalent: `/proc/sys/` + configuration subsystem
//!
//! This module provides the **MECHANISM** for configuration storage.
//! **POLICY** (what to store, validation rules) belongs in modules.
//!
//! # Design
//!
//! - `ConfigValue`: Type-safe primitive values (bool, int, string, array, table)
//! - `Config`: Thread-safe key-value store with flat key access
//! - `ConfigPaths`: XDG-compliant path resolution for config/data/cache directories
//! - `ConfigError`: Error types for config operations
//!
//! # TOML Parsing
//!
//! The kernel does NOT parse TOML directly (no serde dependency).
//! TOML parsing is provided by modules (not the kernel).
//! See [`archive/pre_kernel/lib/core/src/config/loader.rs`](https://github.com/ds1sqe/reovim/blob/81806439/archive/pre_kernel/lib/core/src/config/loader.rs) for the original implementation.
//! This keeps the kernel dependency-free and policy-agnostic.
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::{Config, ConfigValue, ConfigPaths};
//!
//! let config = Config::new();
//!
//! // Set values
//! config.set_str("editor.theme", "dark");
//! config.set_int("editor.tabwidth", 4);
//! config.set_bool("editor.number", true);
//!
//! // Get values
//! assert_eq!(config.get_str("editor.theme"), Some("dark".to_string()));
//! assert_eq!(config.get_int("editor.tabwidth"), Some(4));
//!
//! // Get config paths
//! let config_file = ConfigPaths::config_file()?;
//! ```

use std::{collections::HashMap, fmt, path::PathBuf};

use reovim_arch::sync::RwLock;

// ============================================================================
// ConfigValue - Type-safe configuration values
// ============================================================================

/// Type-safe configuration value.
///
/// This is a kernel-level primitive for configuration storage.
/// Higher layers (`OptionRegistry`, `ProfileManager`) add validation
/// and metadata on top.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// Boolean value.
    Bool(bool),
    /// Integer value (i64 for flexibility).
    Integer(i64),
    /// String value.
    String(String),
    /// Array of values (homogeneous).
    Array(Vec<Self>),
    /// Nested table/section.
    Table(HashMap<String, Self>),
}

impl ConfigValue {
    /// Get the type name for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Integer(_) => "integer",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Table(_) => "table",
        }
    }

    /// Try to get as boolean.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as integer.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as string reference.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as array reference.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get as table reference.
    #[must_use]
    pub const fn as_table(&self) -> Option<&HashMap<String, Self>> {
        match self {
            Self::Table(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get as mutable table reference.
    #[must_use]
    pub const fn as_table_mut(&mut self) -> Option<&mut HashMap<String, Self>> {
        match self {
            Self::Table(t) => Some(t),
            _ => None,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Array(arr) => write!(f, "[{} items]", arr.len()),
            Self::Table(t) => write!(f, "{{{} entries}}", t.len()),
        }
    }
}

// Convenient From implementations
#[cfg_attr(coverage_nightly, coverage(off))]
impl From<bool> for ConfigValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<i64> for ConfigValue {
    fn from(i: i64) -> Self {
        Self::Integer(i)
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<i32> for ConfigValue {
    fn from(i: i32) -> Self {
        Self::Integer(i64::from(i))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<String> for ConfigValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<&str> for ConfigValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<T: Into<Self>> From<Vec<T>> for ConfigValue {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// ConfigError - Error types
// ============================================================================

/// Errors that can occur during configuration operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Key not found in configuration.
    NotFound(String),
    /// Type mismatch when accessing value.
    TypeMismatch {
        /// The key that was accessed
        key: String,
        /// Expected type
        expected: &'static str,
        /// Actual type
        got: &'static str,
    },
    /// Path-related error (missing home, invalid path).
    PathError(String),
    /// IO error (file not found, permission denied).
    Io(String),
    /// Parse error (invalid format).
    Parse(String),
    /// Serialization error.
    Serialize(String),
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(key) => write!(f, "config key not found: {key}"),
            Self::TypeMismatch { key, expected, got } => {
                write!(f, "type mismatch for '{key}': expected {expected}, got {got}")
            }
            Self::PathError(msg) => write!(f, "path error: {msg}"),
            Self::Io(msg) => write!(f, "IO error: {msg}"),
            Self::Parse(msg) => write!(f, "parse error: {msg}"),
            Self::Serialize(msg) => write!(f, "serialize error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

// ============================================================================
// Config - Thread-safe configuration store
// ============================================================================

/// Thread-safe configuration store.
///
/// Provides flat key-value storage with type-safe accessors.
/// Keys use dot notation for logical grouping: `editor.theme`, `plugin.lsp.timeout`
///
/// # Thread Safety
///
/// All operations are thread-safe via internal `RwLock`.
/// Multiple readers allowed, single writer for mutations.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{Config, ConfigValue};
///
/// let config = Config::new();
///
/// // Set values
/// config.set_str("editor.theme", "dark");
/// config.set_int("editor.tabwidth", 4);
///
/// // Get values
/// assert_eq!(config.get_str("editor.theme"), Some("dark".to_string()));
/// assert_eq!(config.get_int("editor.tabwidth"), Some(4));
/// ```
#[derive(Debug, Default)]
pub struct Config {
    /// Root configuration data (flat key-value store).
    data: RwLock<HashMap<String, ConfigValue>>,
    /// Associated file path (if loaded from file).
    path: RwLock<Option<PathBuf>>,
}

impl Config {
    /// Create a new empty configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with an associated file path.
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            path: RwLock::new(Some(path)),
        }
    }

    /// Get the associated file path.
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.path.read().clone()
    }

    /// Set the associated file path.
    pub fn set_path(&self, path: PathBuf) {
        *self.path.write() = Some(path);
    }

    // ========================================================================
    // Get operations
    // ========================================================================

    /// Get a configuration value by key.
    ///
    /// Keys use dot notation: `editor.theme`, `plugin.lsp.timeout`
    #[must_use]
    pub fn get(&self, key: &str) -> Option<ConfigValue> {
        let data = self.data.read();
        data.get(key).cloned()
    }

    /// Get a boolean value.
    #[must_use]
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Get an integer value.
    #[must_use]
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_int())
    }

    /// Get a string value.
    #[must_use]
    pub fn get_str(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str().map(String::from))
    }

    /// Get a value with a default fallback.
    #[must_use]
    pub fn get_or(&self, key: &str, default: ConfigValue) -> ConfigValue {
        self.get(key).unwrap_or(default)
    }

    /// Get a boolean with a default.
    #[must_use]
    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    /// Get an integer with a default.
    #[must_use]
    pub fn get_int_or(&self, key: &str, default: i64) -> i64 {
        self.get_int(key).unwrap_or(default)
    }

    /// Get a string with a default.
    #[must_use]
    pub fn get_str_or(&self, key: &str, default: &str) -> String {
        self.get_str(key).unwrap_or_else(|| default.to_string())
    }

    // ========================================================================
    // Set operations
    // ========================================================================

    /// Set a configuration value.
    ///
    /// Keys use dot notation. Previous value is overwritten.
    pub fn set(&self, key: &str, value: ConfigValue) {
        let mut data = self.data.write();
        data.insert(key.to_string(), value);
    }

    /// Set a boolean value.
    pub fn set_bool(&self, key: &str, value: bool) {
        self.set(key, ConfigValue::Bool(value));
    }

    /// Set an integer value.
    pub fn set_int(&self, key: &str, value: i64) {
        self.set(key, ConfigValue::Integer(value));
    }

    /// Set a string value.
    pub fn set_str(&self, key: &str, value: impl Into<String>) {
        self.set(key, ConfigValue::String(value.into()));
    }

    // ========================================================================
    // Bulk operations
    // ========================================================================

    /// Remove a configuration key.
    ///
    /// Returns the removed value if it existed.
    pub fn remove(&self, key: &str) -> Option<ConfigValue> {
        let mut data = self.data.write();
        data.remove(key)
    }

    /// Check if a key exists.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        let data = self.data.read();
        data.contains_key(key)
    }

    /// Get all keys.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let data = self.data.read();
        data.keys().cloned().collect()
    }

    /// Get all keys matching a prefix.
    #[must_use]
    pub fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let data = self.data.read();
        data.keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Clear all configuration data.
    pub fn clear(&self) {
        let mut data = self.data.write();
        data.clear();
    }

    /// Get the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        let data = self.data.read();
        data.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let data = self.data.read();
        data.is_empty()
    }

    /// Merge another config into this one.
    ///
    /// Values from `other` overwrite values in `self`.
    pub fn merge(&self, other: &Self) {
        let other_data = other.data.read();
        let mut self_data = self.data.write();

        for (key, value) in other_data.iter() {
            self_data.insert(key.clone(), value.clone());
        }
    }

    /// Export all data as a `HashMap`.
    #[must_use]
    pub fn to_map(&self) -> HashMap<String, ConfigValue> {
        let data = self.data.read();
        data.clone()
    }

    /// Import data from a `HashMap`.
    pub fn from_map(&self, map: HashMap<String, ConfigValue>) {
        let mut data = self.data.write();
        *data = map;
    }
}

// ============================================================================
// ConfigPaths - XDG-compliant path resolution
// ============================================================================

/// Configuration path utilities.
///
/// Uses `reovim-arch::dirs` for platform-agnostic path resolution.
/// Follows XDG Base Directory Specification on Unix.
///
/// # Environment Variable Overrides
///
/// All paths can be overridden via environment variables for worktree
/// isolation and testing:
///
/// | Variable | Overrides | Purpose |
/// |----------|-----------|---------|
/// | `REOVIM_CONFIG_DIR` | `config_dir()` | User config (modules.toml, profiles) |
/// | `REOVIM_DATA_DIR` | `data_dir()` | Runtime data (.so modules, lock files, logs) |
/// | `REOVIM_CACHE_DIR` | `cache_dir()` | Cache (derived from data if unset) |
///
/// Resolution order: `$REOVIM_*_DIR` > XDG/platform default.
pub struct ConfigPaths;

impl ConfigPaths {
    /// Get the reovim config directory.
    ///
    /// Resolution: `$REOVIM_CONFIG_DIR` > `$XDG_CONFIG_HOME/reovim` > `~/.config/reovim`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the config directory cannot be determined
    /// and no env override is set.
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        Self::resolve_dir(
            std::env::var("REOVIM_CONFIG_DIR").ok(),
            reovim_arch::dirs::config_dir(),
            "config",
        )
    }

    /// Get the reovim data directory.
    ///
    /// Resolution: `$REOVIM_DATA_DIR` > `$XDG_DATA_HOME/reovim` > `~/.local/share/reovim`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the data directory cannot be determined
    /// and no env override is set.
    pub fn data_dir() -> Result<PathBuf, ConfigError> {
        Self::resolve_dir(
            std::env::var("REOVIM_DATA_DIR").ok(),
            reovim_arch::dirs::data_local_dir(),
            "data",
        )
    }

    /// Get the reovim cache directory.
    ///
    /// Resolution: `$REOVIM_CACHE_DIR` > `$XDG_CACHE_HOME/reovim` > `~/.cache/reovim`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the cache directory cannot be determined
    /// and no env override is set.
    pub fn cache_dir() -> Result<PathBuf, ConfigError> {
        Self::resolve_dir(
            std::env::var("REOVIM_CACHE_DIR").ok(),
            reovim_arch::dirs::cache_dir(),
            "cache",
        )
    }

    /// Resolve a directory path from env override or platform default.
    ///
    /// This is the pure testable core of the path resolution logic.
    /// The env override takes priority; if absent, the platform default
    /// is used with `/reovim` appended.
    fn resolve_dir(
        env_override: Option<String>,
        platform_default: Option<PathBuf>,
        kind: &str,
    ) -> Result<PathBuf, ConfigError> {
        if let Some(dir) = env_override {
            return Ok(PathBuf::from(dir));
        }
        platform_default
            .map(|p| p.join("reovim"))
            .ok_or_else(|| ConfigError::PathError(format!("cannot determine {kind} directory")))
    }

    /// Get the path to the main config file.
    ///
    /// Returns `{config_dir}/config.toml`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the config directory cannot be determined.
    pub fn config_file() -> Result<PathBuf, ConfigError> {
        Self::config_dir().map(|p| p.join("config.toml"))
    }

    /// Get the profiles directory.
    ///
    /// Returns `{config_dir}/profiles/`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the config directory cannot be determined.
    pub fn profiles_dir() -> Result<PathBuf, ConfigError> {
        Self::config_dir().map(|p| p.join("profiles"))
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
