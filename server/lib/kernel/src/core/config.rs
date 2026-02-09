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
//! See `archive/pre_kernel/lib/core/src/config/loader.rs` for the original implementation.
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
pub struct ConfigPaths;

impl ConfigPaths {
    /// Get the reovim config directory.
    ///
    /// - Linux: `$XDG_CONFIG_HOME/reovim` or `~/.config/reovim`
    /// - macOS: `~/Library/Application Support/reovim`
    /// - Windows: `{FOLDERID_RoamingAppData}/reovim`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the config directory cannot be determined.
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        reovim_arch::dirs::config_dir()
            .map(|p| p.join("reovim"))
            .ok_or_else(|| ConfigError::PathError("cannot determine config directory".into()))
    }

    /// Get the reovim data directory.
    ///
    /// - Linux: `$XDG_DATA_HOME/reovim` or `~/.local/share/reovim`
    /// - macOS: `~/Library/Application Support/reovim`
    /// - Windows: `{FOLDERID_LocalAppData}/reovim`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the data directory cannot be determined.
    pub fn data_dir() -> Result<PathBuf, ConfigError> {
        reovim_arch::dirs::data_local_dir()
            .map(|p| p.join("reovim"))
            .ok_or_else(|| ConfigError::PathError("cannot determine data directory".into()))
    }

    /// Get the reovim cache directory.
    ///
    /// - Linux: `$XDG_CACHE_HOME/reovim` or `~/.cache/reovim`
    /// - macOS: `~/Library/Caches/reovim`
    /// - Windows: `{FOLDERID_LocalAppData}/reovim/cache`
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathError` if the cache directory cannot be determined.
    pub fn cache_dir() -> Result<PathBuf, ConfigError> {
        reovim_arch::dirs::cache_dir()
            .map(|p| p.join("reovim"))
            .ok_or_else(|| ConfigError::PathError("cannot determine cache directory".into()))
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ConfigValue tests
    // ========================================================================

    #[test]
    fn test_config_value_types() {
        let bool_val = ConfigValue::Bool(true);
        let int_val = ConfigValue::Integer(42);
        let str_val = ConfigValue::String("hello".into());

        assert_eq!(bool_val.type_name(), "bool");
        assert_eq!(int_val.type_name(), "integer");
        assert_eq!(str_val.type_name(), "string");
    }

    #[test]
    fn test_config_value_conversions() {
        let bool_val = ConfigValue::Bool(true);
        let int_val = ConfigValue::Integer(42);
        let str_val = ConfigValue::String("hello".into());

        assert_eq!(bool_val.as_bool(), Some(true));
        assert_eq!(bool_val.as_int(), None);

        assert_eq!(int_val.as_int(), Some(42));
        assert_eq!(int_val.as_str(), None);

        assert_eq!(str_val.as_str(), Some("hello"));
        assert_eq!(str_val.as_bool(), None);
    }

    #[test]
    fn test_config_value_from_impls() {
        let from_bool: ConfigValue = true.into();
        let from_int: ConfigValue = 42i64.into();
        let from_str: ConfigValue = "hello".into();

        assert_eq!(from_bool.as_bool(), Some(true));
        assert_eq!(from_int.as_int(), Some(42));
        assert_eq!(from_str.as_str(), Some("hello"));
    }

    #[test]
    fn test_config_value_display() {
        assert_eq!(ConfigValue::Bool(true).to_string(), "true");
        assert_eq!(ConfigValue::Integer(42).to_string(), "42");
        assert_eq!(ConfigValue::String("hello".into()).to_string(), "hello");
    }

    // ========================================================================
    // Config tests
    // ========================================================================

    #[test]
    fn test_config_set_get() {
        let config = Config::new();

        config.set_str("editor.theme", "dark");
        config.set_int("editor.tabwidth", 4);
        config.set_bool("editor.number", true);

        assert_eq!(config.get_str("editor.theme"), Some("dark".to_string()));
        assert_eq!(config.get_int("editor.tabwidth"), Some(4));
        assert_eq!(config.get_bool("editor.number"), Some(true));
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::new();

        assert!(config.get_bool_or("missing", true));
        assert_eq!(config.get_int_or("missing", 42), 42);
        assert_eq!(config.get_str_or("missing", "default"), "default");
    }

    #[test]
    fn test_config_keys() {
        let config = Config::new();

        config.set_str("editor.theme", "dark");
        config.set_str("editor.colormode", "truecolor");
        config.set_str("plugin.lsp.timeout", "1000");

        let editor_keys = config.keys_with_prefix("editor");
        assert_eq!(editor_keys.len(), 2);
    }

    #[test]
    fn test_config_merge() {
        let config1 = Config::new();
        config1.set_str("key1", "value1");
        config1.set_str("key2", "value2");

        let config2 = Config::new();
        config2.set_str("key2", "overwritten");
        config2.set_str("key3", "value3");

        config1.merge(&config2);

        assert_eq!(config1.get_str("key1"), Some("value1".to_string()));
        assert_eq!(config1.get_str("key2"), Some("overwritten".to_string()));
        assert_eq!(config1.get_str("key3"), Some("value3".to_string()));
    }

    #[test]
    fn test_config_remove() {
        let config = Config::new();
        config.set_str("key", "value");

        assert!(config.contains("key"));

        let removed = config.remove("key");
        assert!(removed.is_some());
        assert!(!config.contains("key"));
    }

    #[test]
    fn test_config_clear() {
        let config = Config::new();
        config.set_str("key1", "value1");
        config.set_str("key2", "value2");

        assert_eq!(config.len(), 2);

        config.clear();

        assert_eq!(config.len(), 0);
        assert!(config.is_empty());
    }

    #[test]
    fn test_config_with_path() {
        let path = PathBuf::from("/tmp/test.toml");
        let config = Config::with_path(path.clone());

        assert_eq!(config.path(), Some(path));
    }

    #[test]
    fn test_config_to_from_map() {
        let config = Config::new();
        config.set_str("key1", "value1");
        config.set_int("key2", 42);

        let map = config.to_map();
        assert_eq!(map.len(), 2);

        let config2 = Config::new();
        config2.from_map(map);

        assert_eq!(config2.get_str("key1"), Some("value1".to_string()));
        assert_eq!(config2.get_int("key2"), Some(42));
    }

    // ========================================================================
    // ConfigPaths tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_config_paths() {
        // These should succeed on all platforms with a home directory
        let config_dir = ConfigPaths::config_dir();
        let data_dir = ConfigPaths::data_dir();
        let cache_dir = ConfigPaths::cache_dir();

        // In CI/testing environments, these might fail
        // but at least verify the functions don't panic
        if let Ok(path) = config_dir {
            assert!(path.ends_with("reovim"));
        }
        if let Ok(path) = data_dir {
            assert!(path.ends_with("reovim"));
        }
        if let Ok(path) = cache_dir {
            assert!(path.ends_with("reovim"));
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_config_file_path() {
        if let Ok(path) = ConfigPaths::config_file() {
            assert!(path.ends_with("config.toml"));
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_profiles_dir_path() {
        if let Ok(path) = ConfigPaths::profiles_dir() {
            assert!(path.ends_with("profiles"));
        }
    }

    // ========================================================================
    // ConfigError tests
    // ========================================================================

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::NotFound("key".into());
        assert!(err.to_string().contains("key"));

        let err = ConfigError::TypeMismatch {
            key: "key".into(),
            expected: "string",
            got: "integer",
        };
        assert!(err.to_string().contains("type mismatch"));

        let err = ConfigError::PathError("no home".into());
        assert!(err.to_string().contains("path error"));
    }

    // === ConfigValue Array/Table coverage ===

    #[test]
    fn test_config_value_array() {
        let arr = ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]);
        assert_eq!(arr.type_name(), "array");
        assert!(arr.as_array().is_some());
        assert_eq!(arr.as_array().unwrap().len(), 2);
        assert!(arr.as_bool().is_none());
        assert!(arr.as_int().is_none());
        assert!(arr.as_str().is_none());
        assert!(arr.as_table().is_none());
    }

    #[test]
    fn test_config_value_table() {
        let mut map = HashMap::new();
        map.insert("key".to_string(), ConfigValue::Bool(true));
        let table = ConfigValue::Table(map);
        assert_eq!(table.type_name(), "table");
        assert!(table.as_table().is_some());
        assert_eq!(table.as_table().unwrap().len(), 1);
        assert!(table.as_bool().is_none());
    }

    #[test]
    fn test_config_value_table_mut() {
        let mut map = HashMap::new();
        map.insert("k".to_string(), ConfigValue::Integer(1));
        let mut table = ConfigValue::Table(map);
        assert!(table.as_table_mut().is_some());
        table
            .as_table_mut()
            .unwrap()
            .insert("k2".to_string(), ConfigValue::Integer(2));
        assert_eq!(table.as_table().unwrap().len(), 2);

        // Non-table returns None
        let mut non_table = ConfigValue::Bool(true);
        assert!(non_table.as_table_mut().is_none());
    }

    #[test]
    fn test_config_value_display_array_table() {
        let arr = ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]);
        assert_eq!(arr.to_string(), "[2 items]");

        let mut map = HashMap::new();
        map.insert("a".to_string(), ConfigValue::Bool(true));
        let table = ConfigValue::Table(map);
        assert_eq!(table.to_string(), "{1 entries}");
    }

    // === From impls coverage ===

    #[test]
    fn test_config_value_from_i32() {
        let val: ConfigValue = 42i32.into();
        assert_eq!(val.as_int(), Some(42));
    }

    #[test]
    fn test_config_value_from_string_owned() {
        let val: ConfigValue = String::from("owned").into();
        assert_eq!(val.as_str(), Some("owned"));
    }

    #[test]
    fn test_config_value_from_vec() {
        let val: ConfigValue = vec![1i64, 2i64, 3i64].into();
        assert!(val.as_array().is_some());
        assert_eq!(val.as_array().unwrap().len(), 3);
    }

    // === ConfigError remaining variants ===

    #[test]
    fn test_config_error_io_parse_serialize() {
        let err = ConfigError::Io("disk full".into());
        assert!(err.to_string().contains("IO error"));
        assert!(err.to_string().contains("disk full"));

        let err = ConfigError::Parse("bad syntax".into());
        assert!(err.to_string().contains("parse error"));
        assert!(err.to_string().contains("bad syntax"));

        let err = ConfigError::Serialize("encode fail".into());
        assert!(err.to_string().contains("serialize error"));
        assert!(err.to_string().contains("encode fail"));
    }

    #[test]
    fn test_config_error_is_error_trait() {
        let err: Box<dyn std::error::Error> = Box::new(ConfigError::NotFound("x".into()));
        assert!(err.to_string().contains('x'));
    }

    // === Config set_path / get_or ===

    #[test]
    fn test_config_set_path() {
        let config = Config::new();
        assert!(config.path().is_none());

        config.set_path(PathBuf::from("/new/path.toml"));
        assert_eq!(config.path(), Some(PathBuf::from("/new/path.toml")));
    }

    #[test]
    fn test_config_get_or() {
        let config = Config::new();
        config.set_int("exists", 42);

        let val = config.get_or("exists", ConfigValue::Integer(0));
        assert_eq!(val.as_int(), Some(42));

        let val = config.get_or("missing", ConfigValue::Integer(99));
        assert_eq!(val.as_int(), Some(99));
    }

    // ========================================================================
    // Additional coverage tests
    // ========================================================================

    #[test]
    fn test_config_value_bool_false() {
        let val = ConfigValue::Bool(false);
        assert_eq!(val.as_bool(), Some(false));
        assert_eq!(val.to_string(), "false");
    }

    #[test]
    fn test_config_value_negative_integer() {
        let val = ConfigValue::Integer(-100);
        assert_eq!(val.as_int(), Some(-100));
        assert_eq!(val.to_string(), "-100");
    }

    #[test]
    fn test_config_value_empty_string() {
        let val = ConfigValue::String(String::new());
        assert_eq!(val.as_str(), Some(""));
        assert_eq!(val.to_string(), "");
    }

    #[test]
    fn test_config_value_empty_array() {
        let arr = ConfigValue::Array(Vec::new());
        assert_eq!(arr.type_name(), "array");
        assert_eq!(arr.as_array().unwrap().len(), 0);
        assert_eq!(arr.to_string(), "[0 items]");
        assert!(arr.as_bool().is_none());
        assert!(arr.as_int().is_none());
        assert!(arr.as_str().is_none());
        assert!(arr.as_table().is_none());
    }

    #[test]
    fn test_config_value_empty_table() {
        let table = ConfigValue::Table(HashMap::new());
        assert_eq!(table.type_name(), "table");
        assert_eq!(table.as_table().unwrap().len(), 0);
        assert_eq!(table.to_string(), "{0 entries}");
        assert!(table.as_bool().is_none());
        assert!(table.as_int().is_none());
        assert!(table.as_str().is_none());
        assert!(table.as_array().is_none());
    }

    #[test]
    fn test_config_value_nested_array() {
        let inner = ConfigValue::Array(vec![ConfigValue::Bool(true)]);
        let outer = ConfigValue::Array(vec![inner.clone()]);
        assert_eq!(outer.as_array().unwrap().len(), 1);
        assert_eq!(outer.as_array().unwrap()[0], inner);
    }

    #[test]
    fn test_config_value_nested_table() {
        let mut inner = HashMap::new();
        inner.insert("nested".to_string(), ConfigValue::Integer(42));
        let inner_table = ConfigValue::Table(inner);

        let mut outer = HashMap::new();
        outer.insert("child".to_string(), inner_table);
        let outer_table = ConfigValue::Table(outer);

        let child = outer_table.as_table().unwrap().get("child").unwrap();
        assert_eq!(child.as_table().unwrap().get("nested").unwrap().as_int(), Some(42));
    }

    #[test]
    fn test_config_value_table_mut_insert_and_read() {
        let mut table = ConfigValue::Table(HashMap::new());
        let t = table.as_table_mut().unwrap();
        t.insert("a".to_string(), ConfigValue::Bool(true));
        t.insert("b".to_string(), ConfigValue::Integer(99));
        t.insert("c".to_string(), ConfigValue::String("hello".into()));

        assert_eq!(table.as_table().unwrap().len(), 3);
        assert_eq!(table.as_table().unwrap().get("a").unwrap().as_bool(), Some(true));
        assert_eq!(table.as_table().unwrap().get("b").unwrap().as_int(), Some(99));
        assert_eq!(table.as_table().unwrap().get("c").unwrap().as_str(), Some("hello"));
    }

    #[test]
    fn test_config_value_from_vec_of_strings() {
        let val: ConfigValue = vec!["a".to_string(), "b".to_string()].into();
        assert!(val.as_array().is_some());
        assert_eq!(val.as_array().unwrap().len(), 2);
        assert_eq!(val.as_array().unwrap()[0].as_str(), Some("a"));
    }

    #[test]
    fn test_config_value_from_vec_empty() {
        let val: ConfigValue = Vec::<i64>::new().into();
        assert!(val.as_array().is_some());
        assert_eq!(val.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_config_value_from_bool_false() {
        let val: ConfigValue = false.into();
        assert_eq!(val.as_bool(), Some(false));
    }

    #[test]
    fn test_config_value_from_negative_i64() {
        let val: ConfigValue = (-42i64).into();
        assert_eq!(val.as_int(), Some(-42));
    }

    #[test]
    fn test_config_value_from_negative_i32() {
        let val: ConfigValue = (-10i32).into();
        assert_eq!(val.as_int(), Some(-10));
    }

    #[test]
    fn test_config_value_from_zero_i32() {
        let val: ConfigValue = 0i32.into();
        assert_eq!(val.as_int(), Some(0));
    }

    #[test]
    fn test_config_value_as_array_on_non_array() {
        assert!(ConfigValue::Bool(true).as_array().is_none());
        assert!(ConfigValue::Integer(1).as_array().is_none());
        assert!(ConfigValue::String("s".into()).as_array().is_none());
        assert!(ConfigValue::Table(HashMap::new()).as_array().is_none());
    }

    #[test]
    fn test_config_value_as_table_on_non_table() {
        assert!(ConfigValue::Bool(true).as_table().is_none());
        assert!(ConfigValue::Integer(1).as_table().is_none());
        assert!(ConfigValue::String("s".into()).as_table().is_none());
        assert!(ConfigValue::Array(Vec::new()).as_table().is_none());
    }

    #[test]
    fn test_config_value_as_table_mut_on_non_table() {
        let mut b = ConfigValue::Bool(true);
        let mut i = ConfigValue::Integer(1);
        let mut s = ConfigValue::String("s".into());
        let mut a = ConfigValue::Array(Vec::new());
        assert!(b.as_table_mut().is_none());
        assert!(i.as_table_mut().is_none());
        assert!(s.as_table_mut().is_none());
        assert!(a.as_table_mut().is_none());
    }

    #[test]
    fn test_config_value_as_bool_on_all_types() {
        assert!(ConfigValue::Integer(1).as_bool().is_none());
        assert!(ConfigValue::String("s".into()).as_bool().is_none());
        assert!(ConfigValue::Array(Vec::new()).as_bool().is_none());
        assert!(ConfigValue::Table(HashMap::new()).as_bool().is_none());
    }

    #[test]
    fn test_config_value_as_int_on_all_types() {
        assert!(ConfigValue::Bool(true).as_int().is_none());
        assert!(ConfigValue::String("s".into()).as_int().is_none());
        assert!(ConfigValue::Array(Vec::new()).as_int().is_none());
        assert!(ConfigValue::Table(HashMap::new()).as_int().is_none());
    }

    #[test]
    fn test_config_value_as_str_on_all_types() {
        assert!(ConfigValue::Bool(true).as_str().is_none());
        assert!(ConfigValue::Integer(1).as_str().is_none());
        assert!(ConfigValue::Array(Vec::new()).as_str().is_none());
        assert!(ConfigValue::Table(HashMap::new()).as_str().is_none());
    }

    #[test]
    fn test_config_value_equality() {
        assert_eq!(ConfigValue::Bool(true), ConfigValue::Bool(true));
        assert_ne!(ConfigValue::Bool(true), ConfigValue::Bool(false));
        assert_ne!(ConfigValue::Bool(true), ConfigValue::Integer(1));
        assert_eq!(ConfigValue::Integer(42), ConfigValue::Integer(42));
        assert_ne!(ConfigValue::Integer(42), ConfigValue::Integer(43));
        assert_eq!(ConfigValue::String("a".into()), ConfigValue::String("a".into()));
        assert_ne!(ConfigValue::String("a".into()), ConfigValue::String("b".into()));
    }

    #[test]
    fn test_config_value_clone() {
        let original =
            ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::String("two".into())]);
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_config_value_debug() {
        let val = ConfigValue::Bool(true);
        let debug = format!("{val:?}");
        assert!(debug.contains("Bool"));
        assert!(debug.contains("true"));

        let val = ConfigValue::Integer(42);
        let debug = format!("{val:?}");
        assert!(debug.contains("Integer"));
        assert!(debug.contains("42"));

        let val = ConfigValue::String("hello".into());
        let debug = format!("{val:?}");
        assert!(debug.contains("String"));
        assert!(debug.contains("hello"));

        let val = ConfigValue::Array(vec![ConfigValue::Integer(1)]);
        let debug = format!("{val:?}");
        assert!(debug.contains("Array"));

        let val = ConfigValue::Table(HashMap::new());
        let debug = format!("{val:?}");
        assert!(debug.contains("Table"));
    }

    #[test]
    fn test_config_get_nonexistent() {
        let config = Config::new();
        assert!(config.get("nonexistent").is_none());
        assert!(config.get_bool("nonexistent").is_none());
        assert!(config.get_int("nonexistent").is_none());
        assert!(config.get_str("nonexistent").is_none());
    }

    #[test]
    fn test_config_get_type_mismatch() {
        let config = Config::new();
        config.set_str("key", "value");

        // Trying to get a string as bool/int returns None
        assert!(config.get_bool("key").is_none());
        assert!(config.get_int("key").is_none());

        config.set_bool("flag", true);
        assert!(config.get_int("flag").is_none());
        assert!(config.get_str("flag").is_none());

        config.set_int("num", 42);
        assert!(config.get_bool("num").is_none());
        assert!(config.get_str("num").is_none());
    }

    #[test]
    fn test_config_get_bool_or_with_existing_value() {
        let config = Config::new();
        config.set_bool("flag", false);
        assert!(!config.get_bool_or("flag", true));
    }

    #[test]
    fn test_config_get_int_or_with_existing_value() {
        let config = Config::new();
        config.set_int("num", 10);
        assert_eq!(config.get_int_or("num", 99), 10);
    }

    #[test]
    fn test_config_get_str_or_with_existing_value() {
        let config = Config::new();
        config.set_str("name", "alice");
        assert_eq!(config.get_str_or("name", "bob"), "alice");
    }

    #[test]
    fn test_config_set_overwrites() {
        let config = Config::new();
        config.set_str("key", "first");
        assert_eq!(config.get_str("key"), Some("first".to_string()));

        config.set_str("key", "second");
        assert_eq!(config.get_str("key"), Some("second".to_string()));
    }

    #[test]
    fn test_config_set_different_types() {
        let config = Config::new();
        // Set as string, then overwrite with int
        config.set_str("key", "text");
        config.set_int("key", 42);
        assert_eq!(config.get_int("key"), Some(42));
        assert!(config.get_str("key").is_none());
    }

    #[test]
    fn test_config_set_generic_value() {
        let config = Config::new();
        let arr = ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]);
        config.set("arr_key", arr.clone());
        assert_eq!(config.get("arr_key"), Some(arr));

        let mut map = HashMap::new();
        map.insert("inner".to_string(), ConfigValue::Bool(true));
        let table = ConfigValue::Table(map);
        config.set("table_key", table.clone());
        assert_eq!(config.get("table_key"), Some(table));
    }

    #[test]
    fn test_config_remove_nonexistent() {
        let config = Config::new();
        let removed = config.remove("nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_config_remove_returns_value() {
        let config = Config::new();
        config.set_int("key", 42);
        let removed = config.remove("key");
        assert_eq!(removed, Some(ConfigValue::Integer(42)));
        // Verify it's actually gone
        assert!(config.get("key").is_none());
    }

    #[test]
    fn test_config_contains_empty() {
        let config = Config::new();
        assert!(!config.contains("anything"));
    }

    #[test]
    fn test_config_keys_empty() {
        let config = Config::new();
        assert!(config.keys().is_empty());
    }

    #[test]
    fn test_config_keys_returns_all() {
        let config = Config::new();
        config.set_str("a", "1");
        config.set_str("b", "2");
        config.set_str("c", "3");

        let keys = config.keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
        assert!(keys.contains(&"c".to_string()));
    }

    #[test]
    fn test_config_keys_with_prefix_no_match() {
        let config = Config::new();
        config.set_str("editor.theme", "dark");
        config.set_str("editor.font", "mono");

        let keys = config.keys_with_prefix("plugin");
        assert!(keys.is_empty());
    }

    #[test]
    fn test_config_keys_with_prefix_all_match() {
        let config = Config::new();
        config.set_str("editor.theme", "dark");
        config.set_str("editor.font", "mono");
        config.set_str("editor.tabwidth", "4");

        let keys = config.keys_with_prefix("editor");
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_config_clear_already_empty() {
        let config = Config::new();
        config.clear();
        assert!(config.is_empty());
        assert_eq!(config.len(), 0);
    }

    #[test]
    fn test_config_len_and_is_empty() {
        let config = Config::new();
        assert!(config.is_empty());
        assert_eq!(config.len(), 0);

        config.set_str("k1", "v1");
        assert!(!config.is_empty());
        assert_eq!(config.len(), 1);

        config.set_str("k2", "v2");
        assert_eq!(config.len(), 2);

        config.remove("k1");
        assert_eq!(config.len(), 1);
    }

    #[test]
    fn test_config_merge_empty_into_empty() {
        let config1 = Config::new();
        let config2 = Config::new();
        config1.merge(&config2);
        assert!(config1.is_empty());
    }

    #[test]
    fn test_config_merge_into_empty() {
        let config1 = Config::new();
        let config2 = Config::new();
        config2.set_str("key", "value");
        config1.merge(&config2);
        assert_eq!(config1.get_str("key"), Some("value".to_string()));
    }

    #[test]
    fn test_config_merge_empty_into_non_empty() {
        let config1 = Config::new();
        config1.set_str("key", "value");
        let config2 = Config::new();
        config1.merge(&config2);
        assert_eq!(config1.get_str("key"), Some("value".to_string()));
    }

    #[test]
    fn test_config_to_map_empty() {
        let config = Config::new();
        let map = config.to_map();
        assert!(map.is_empty());
    }

    #[test]
    fn test_config_from_map_overwrites() {
        let config = Config::new();
        config.set_str("old_key", "old_value");

        let mut map = HashMap::new();
        map.insert("new_key".to_string(), ConfigValue::Integer(42));
        config.from_map(map);

        assert!(config.get("old_key").is_none());
        assert_eq!(config.get_int("new_key"), Some(42));
    }

    #[test]
    fn test_config_from_map_empty() {
        let config = Config::new();
        config.set_str("key", "value");
        config.from_map(HashMap::new());
        assert!(config.is_empty());
    }

    #[test]
    fn test_config_with_path_operations() {
        let path = PathBuf::from("/tmp/test.toml");
        let config = Config::with_path(path.clone());
        assert_eq!(config.path(), Some(path));
        assert!(config.is_empty()); // No data, just path

        // Can set data on a path-associated config
        config.set_str("key", "value");
        assert_eq!(config.get_str("key"), Some("value".to_string()));
    }

    #[test]
    fn test_config_new_has_no_path() {
        let config = Config::new();
        assert!(config.path().is_none());
    }

    #[test]
    fn test_config_set_path_overwrite() {
        let config = Config::new();
        config.set_path(PathBuf::from("/first.toml"));
        config.set_path(PathBuf::from("/second.toml"));
        assert_eq!(config.path(), Some(PathBuf::from("/second.toml")));
    }

    #[test]
    fn test_config_get_or_with_array() {
        let config = Config::new();
        let default = ConfigValue::Array(vec![ConfigValue::Integer(1)]);
        let val = config.get_or("missing", default.clone());
        assert_eq!(val, default);
    }

    #[test]
    fn test_config_get_or_with_table() {
        let config = Config::new();
        let mut map = HashMap::new();
        map.insert("k".to_string(), ConfigValue::Bool(true));
        let default = ConfigValue::Table(map);
        let val = config.get_or("missing", default.clone());
        assert_eq!(val, default);
    }

    #[test]
    fn test_config_set_str_with_string_owned() {
        let config = Config::new();
        config.set_str("key", String::from("owned_string"));
        assert_eq!(config.get_str("key"), Some("owned_string".to_string()));
    }

    #[test]
    fn test_config_error_not_found_display() {
        let err = ConfigError::NotFound("editor.theme".into());
        let msg = err.to_string();
        assert!(msg.contains("config key not found"));
        assert!(msg.contains("editor.theme"));
    }

    #[test]
    fn test_config_error_type_mismatch_display() {
        let err = ConfigError::TypeMismatch {
            key: "tabwidth".into(),
            expected: "integer",
            got: "string",
        };
        let msg = err.to_string();
        assert!(msg.contains("type mismatch"));
        assert!(msg.contains("tabwidth"));
        assert!(msg.contains("integer"));
        assert!(msg.contains("string"));
    }

    #[test]
    fn test_config_error_path_error_display() {
        let err = ConfigError::PathError("cannot determine home".into());
        let msg = err.to_string();
        assert!(msg.contains("path error"));
        assert!(msg.contains("cannot determine home"));
    }

    #[test]
    fn test_config_error_debug() {
        let err = ConfigError::NotFound("test".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("NotFound"));

        let err = ConfigError::TypeMismatch {
            key: "k".into(),
            expected: "bool",
            got: "int",
        };
        let debug = format!("{err:?}");
        assert!(debug.contains("TypeMismatch"));

        let err = ConfigError::PathError("msg".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("PathError"));

        let err = ConfigError::Io("io".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("Io"));

        let err = ConfigError::Parse("parse".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("Parse"));

        let err = ConfigError::Serialize("ser".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("Serialize"));
    }

    #[test]
    fn test_config_error_clone() {
        let err = ConfigError::NotFound("key".into());
        let cloned = err.clone();
        assert_eq!(err, cloned);

        let err = ConfigError::TypeMismatch {
            key: "k".into(),
            expected: "bool",
            got: "int",
        };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_config_error_eq() {
        let err1 = ConfigError::NotFound("a".into());
        let err2 = ConfigError::NotFound("a".into());
        let err3 = ConfigError::NotFound("b".into());
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);

        let err4 = ConfigError::Io("msg".into());
        assert_ne!(err1, err4);
    }

    #[test]
    fn test_config_error_all_variants_are_error() {
        let errors: Vec<Box<dyn std::error::Error>> = vec![
            Box::new(ConfigError::NotFound("k".into())),
            Box::new(ConfigError::TypeMismatch {
                key: "k".into(),
                expected: "a",
                got: "b",
            }),
            Box::new(ConfigError::PathError("p".into())),
            Box::new(ConfigError::Io("io".into())),
            Box::new(ConfigError::Parse("parse".into())),
            Box::new(ConfigError::Serialize("ser".into())),
        ];
        for err in &errors {
            // Just verify Display works for all variants
            let _ = err.to_string();
        }
        assert_eq!(errors.len(), 6);
    }

    #[test]
    fn test_config_value_display_large_array() {
        let arr = ConfigValue::Array((0..100).map(ConfigValue::Integer).collect());
        assert_eq!(arr.to_string(), "[100 items]");
    }

    #[test]
    fn test_config_value_display_large_table() {
        let mut map = HashMap::new();
        for i in 0..50 {
            map.insert(format!("key{i}"), ConfigValue::Integer(i));
        }
        let table = ConfigValue::Table(map);
        assert_eq!(table.to_string(), "{50 entries}");
    }

    #[test]
    fn test_config_dot_notation_keys() {
        let config = Config::new();
        config.set_str("a.b.c.d", "deep");
        config.set_str("a.b.e", "sibling");
        config.set_str("x.y", "other");

        assert_eq!(config.get_str("a.b.c.d"), Some("deep".to_string()));
        assert_eq!(config.get_str("a.b.e"), Some("sibling".to_string()));

        let keys = config.keys_with_prefix("a.b");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_config_merge_preserves_types() {
        let config1 = Config::new();
        config1.set_bool("flag", true);
        config1.set_int("count", 5);

        let config2 = Config::new();
        config2.set_str("name", "test");
        let arr = ConfigValue::Array(vec![ConfigValue::Integer(1)]);
        config2.set("list", arr);

        config1.merge(&config2);

        assert_eq!(config1.get_bool("flag"), Some(true));
        assert_eq!(config1.get_int("count"), Some(5));
        assert_eq!(config1.get_str("name"), Some("test".to_string()));
        assert!(config1.get("list").unwrap().as_array().is_some());
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.is_empty());
        assert!(config.path().is_none());
    }

    #[test]
    fn test_config_debug() {
        let config = Config::new();
        config.set_str("key", "value");
        let debug = format!("{config:?}");
        assert!(debug.contains("Config"));
    }
}
