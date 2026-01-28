//! User keymap configuration from `keymap.toml`.
//!
//! Allows users to override keybindings, remove bindings, and select resolvers.
//!
//! # Configuration File Location
//!
//! - Linux: `~/.config/reovim/keymap.toml`
//! - macOS: `~/Library/Application Support/reovim/keymap.toml`
//! - Windows: `%APPDATA%\reovim\keymap.toml`
//!
//! # Configuration Format
//!
//! ```toml
//! # Add/override bindings (User layer - highest priority)
//! [bindings.normal]
//! "<C-s>" = "buffer:save"
//! "<C-q>" = "app:quit"
//!
//! [bindings.insert]
//! "jk" = "mode:normal"   # Quick escape
//!
//! # Remove bindings
//! [remove.normal]
//! keys = ["Q", "gQ"]     # Disable Q and gQ in normal mode
//!
//! # Resolver selection per mode (advanced)
//! [resolvers]
//! normal = "vim"         # default - waits for longer sequences
//! # game = "eager"       # Execute immediately on match
//! ```
//!
//! # Layer Priority
//!
//! User bindings have the highest priority:
//! 1. **User** (highest): From `keymap.toml` - your overrides
//! 2. **Policy**: From modules (Vim, Emacs, etc.)
//! 3. **Base** (lowest): Mechanism defaults

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use {
    reovim_driver_input::{BindingLayer, KeySequence},
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
    serde::Deserialize,
};

use crate::server::registry::KeymapRegistry;

/// User keymap configuration from `keymap.toml`.
///
/// # Example
///
/// ```ignore
/// let config = KeymapConfig::load()?;
/// let stats = config.apply(&mut keymap_registry)?;
/// println!("Added {} bindings", stats.bindings_added);
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
pub struct KeymapConfig {
    /// Binding overrides by mode name.
    ///
    /// Keys are mode names (e.g., "normal", "insert", "visual").
    /// Values are maps of key sequences to command IDs.
    #[serde(default)]
    pub bindings: HashMap<String, ModeBindings>,

    /// Bindings to remove by mode name.
    ///
    /// Keys are mode names. Values contain lists of key sequences
    /// to disable in that mode.
    #[serde(default)]
    pub remove: HashMap<String, RemoveBindings>,

    /// Resolver selection per mode.
    ///
    /// Keys are mode names. Values are resolver names:
    /// - "vim": Wait for longer sequences (default)
    /// - "eager": Execute immediately on match
    #[serde(default)]
    pub resolvers: HashMap<String, String>,
}

/// Bindings for a specific mode.
///
/// This is a transparent wrapper around a `HashMap` to support
/// TOML's table syntax: `[bindings.normal]` followed by `"key" = "command"`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct ModeBindings {
    /// Map of key sequence strings to command ID strings.
    pub bindings: HashMap<String, String>,
}

/// Keys to remove for a specific mode.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoveBindings {
    /// List of key sequence strings to disable.
    pub keys: Vec<String>,
}

/// Statistics from applying keymap configuration.
#[derive(Debug, Default)]
pub struct ApplyStats {
    /// Number of bindings added to the User layer.
    pub bindings_added: usize,
    /// Number of bindings marked as removed.
    pub bindings_removed: usize,
    /// Number of resolver selections applied.
    pub resolvers_set: usize,
}

/// Error type for keymap configuration.
#[derive(Debug)]
pub enum KeymapConfigError {
    /// Config directory not found.
    NoConfigDir,
    /// IO error reading file.
    Io(io::Error),
    /// TOML parse error with optional location.
    Parse {
        /// Error message.
        message: String,
        /// Line number (if available).
        line: Option<usize>,
        /// Column number (if available).
        column: Option<usize>,
    },
    /// Invalid key sequence in configuration.
    InvalidKeySequence {
        /// The invalid key string.
        keys: String,
        /// The mode where this appeared.
        mode: String,
    },
    /// Invalid command ID in configuration.
    InvalidCommand {
        /// The invalid command string.
        command: String,
        /// The mode where this appeared.
        mode: String,
    },
    /// Unknown resolver name.
    UnknownResolver {
        /// The unknown resolver name.
        name: String,
        /// The mode where this appeared.
        mode: String,
    },
}

impl std::fmt::Display for KeymapConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoConfigDir => write!(f, "cannot determine config directory"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Parse {
                message,
                line,
                column,
            } => {
                write!(f, "parse error: {message}")?;
                if let Some(l) = line {
                    write!(f, " at line {l}")?;
                    if let Some(c) = column {
                        write!(f, ", column {c}")?;
                    }
                }
                Ok(())
            }
            Self::InvalidKeySequence { keys, mode } => {
                write!(f, "invalid key sequence '{keys}' in mode '{mode}'")
            }
            Self::InvalidCommand { command, mode } => {
                write!(f, "invalid command '{command}' in mode '{mode}'")
            }
            Self::UnknownResolver { name, mode } => {
                write!(f, "unknown resolver '{name}' for mode '{mode}'")
            }
        }
    }
}

impl std::error::Error for KeymapConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::NoConfigDir
            | Self::Parse { .. }
            | Self::InvalidKeySequence { .. }
            | Self::InvalidCommand { .. }
            | Self::UnknownResolver { .. } => None,
        }
    }
}

impl From<io::Error> for KeymapConfigError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for KeymapConfigError {
    fn from(e: toml::de::Error) -> Self {
        let span = e.span();
        Self::Parse {
            message: e.message().to_string(),
            line: span.map(|s| s.start),
            column: None,
        }
    }
}

impl KeymapConfig {
    /// Load keymap config from the default location.
    ///
    /// Returns default (empty) config if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config directory cannot be determined
    /// - The file exists but cannot be read
    /// - The file contains invalid TOML
    pub fn load() -> Result<Self, KeymapConfigError> {
        let path = keymap_config_path()?;
        Self::load_from(&path)
    }

    /// Load keymap config from a specific path.
    ///
    /// Returns default (empty) config if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_from(path: &Path) -> Result<Self, KeymapConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Check if the configuration has any bindings.
    #[must_use]
    pub fn has_bindings(&self) -> bool {
        self.bindings.values().any(|m| !m.bindings.is_empty())
    }

    /// Check if the configuration has any removals.
    #[must_use]
    pub fn has_removals(&self) -> bool {
        self.remove.values().any(|r| !r.keys.is_empty())
    }

    /// Check if the configuration has any resolver selections.
    #[must_use]
    pub fn has_resolvers(&self) -> bool {
        !self.resolvers.is_empty()
    }

    /// Check if the configuration is empty (no changes).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.has_bindings() && !self.has_removals() && !self.has_resolvers()
    }

    /// Get bindings for a specific mode.
    #[must_use]
    pub fn bindings_for_mode(&self, mode: &str) -> Option<&ModeBindings> {
        self.bindings.get(mode)
    }

    /// Get removals for a specific mode.
    #[must_use]
    pub fn removals_for_mode(&self, mode: &str) -> Option<&RemoveBindings> {
        self.remove.get(mode)
    }

    /// Get resolver selection for a specific mode.
    #[must_use]
    pub fn resolver_for_mode(&self, mode: &str) -> Option<&str> {
        self.resolvers.get(mode).map(String::as_str)
    }

    /// Get all mode names that have bindings.
    pub fn binding_modes(&self) -> impl Iterator<Item = &str> {
        self.bindings
            .iter()
            .filter(|(_, m)| !m.bindings.is_empty())
            .map(|(k, _)| k.as_str())
    }

    /// Get all mode names that have removals.
    pub fn removal_modes(&self) -> impl Iterator<Item = &str> {
        self.remove
            .iter()
            .filter(|(_, r)| !r.keys.is_empty())
            .map(|(k, _)| k.as_str())
    }

    // ========================================================================
    // Validation
    // ========================================================================

    /// Validate configuration without applying it.
    ///
    /// Returns a list of all validation errors found. Use this to report
    /// warnings before applying the config.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = KeymapConfig::load()?;
    /// let errors = config.validate();
    /// for err in &errors {
    ///     warn!("keymap.toml: {}", err);
    /// }
    /// // Still apply valid parts
    /// config.apply(&mut registry)?;
    /// ```
    #[must_use]
    pub fn validate(&self) -> Vec<KeymapConfigError> {
        let mut errors = Vec::new();

        // Validate bindings
        for (mode, mode_bindings) in &self.bindings {
            for (keys, command) in &mode_bindings.bindings {
                // Validate key sequence
                if KeySequence::parse(keys).is_none() {
                    errors.push(KeymapConfigError::InvalidKeySequence {
                        keys: keys.clone(),
                        mode: mode.clone(),
                    });
                }

                // Validate command ID format
                if parse_command_id(command).is_none() {
                    errors.push(KeymapConfigError::InvalidCommand {
                        command: command.clone(),
                        mode: mode.clone(),
                    });
                }
            }
        }

        // Validate remove keys
        for (mode, remove_config) in &self.remove {
            for keys in &remove_config.keys {
                if KeySequence::parse(keys).is_none() {
                    errors.push(KeymapConfigError::InvalidKeySequence {
                        keys: keys.clone(),
                        mode: mode.clone(),
                    });
                }
            }
        }

        // Note: Resolver validation is deferred - infrastructure not ready
        // Future: validate resolver names against known resolvers

        errors
    }

    // ========================================================================
    // Apply configuration to registry
    // ========================================================================

    /// Apply this configuration to a keymap registry.
    ///
    /// Registers all bindings at the User layer (highest priority) and
    /// marks removed bindings as disabled.
    ///
    /// # Arguments
    ///
    /// * `registry` - The keymap registry to modify
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A key sequence string is invalid (cannot be parsed)
    /// - A command ID string is invalid (missing `module:command` format)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = KeymapConfig::load()?;
    /// let stats = config.apply(&mut keymap_registry)?;
    /// info!("Applied {} bindings, removed {}", stats.bindings_added, stats.bindings_removed);
    /// ```
    pub fn apply(&self, registry: &mut KeymapRegistry) -> Result<ApplyStats, KeymapConfigError> {
        let mut stats = ApplyStats::default();

        // Apply binding overrides at User layer
        for (mode_name, mode_bindings) in &self.bindings {
            let mode_id = normalize_mode_id(mode_name);

            for (keys_str, command_str) in &mode_bindings.bindings {
                // Parse key sequence
                let keys = KeySequence::parse(keys_str).ok_or_else(|| {
                    KeymapConfigError::InvalidKeySequence {
                        keys: keys_str.clone(),
                        mode: mode_name.clone(),
                    }
                })?;

                // Parse command ID
                let command = parse_command_id(command_str).ok_or_else(|| {
                    KeymapConfigError::InvalidCommand {
                        command: command_str.clone(),
                        mode: mode_name.clone(),
                    }
                })?;

                registry.register_at_layer(BindingLayer::User, &mode_id, keys, command);
                stats.bindings_added += 1;
            }
        }

        // Apply removals at User layer
        for (mode_name, remove_config) in &self.remove {
            let mode_id = normalize_mode_id(mode_name);

            for keys_str in &remove_config.keys {
                let keys = KeySequence::parse(keys_str).ok_or_else(|| {
                    KeymapConfigError::InvalidKeySequence {
                        keys: keys_str.clone(),
                        mode: mode_name.clone(),
                    }
                })?;

                registry.remove_at_layer(BindingLayer::User, &mode_id, keys);
                stats.bindings_removed += 1;
            }
        }

        Ok(stats)
    }
}

/// Normalize a mode name to a full `ModeId`.
///
/// Supports both short form ("normal") and full form ("editor:normal").
/// Short forms are assumed to belong to the "editor" module.
fn normalize_mode_id(mode_name: &str) -> ModeId {
    if mode_name.contains(':') {
        // Full form: "module:name" - parse and create
        let parts: Vec<&str> = mode_name.splitn(2, ':').collect();
        if parts.len() == 2 {
            let module: &'static str = Box::leak(parts[0].to_string().into_boxed_str());
            let name: &'static str = Box::leak(parts[1].to_string().into_boxed_str());
            ModeId::new(ModuleId::new(module), name)
        } else {
            // Fallback to editor module
            let name: &'static str = Box::leak(mode_name.to_string().into_boxed_str());
            ModeId::new(ModuleId::new("editor"), name)
        }
    } else {
        // Short form: "normal" -> "editor:normal"
        let name: &'static str = Box::leak(mode_name.to_string().into_boxed_str());
        ModeId::new(ModuleId::new("editor"), name)
    }
}

/// Parse a command ID from a string.
///
/// Expects "module:command" format (e.g., "editor:cursor-down").
fn parse_command_id(command_str: &str) -> Option<CommandId> {
    let parts: Vec<&str> = command_str.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }

    let module: &'static str = Box::leak(parts[0].to_string().into_boxed_str());
    let name: &'static str = Box::leak(parts[1].to_string().into_boxed_str());
    Some(CommandId::new(ModuleId::new(module), name))
}

/// Get the path to the keymap configuration file.
///
/// Returns `~/.config/reovim/keymap.toml` on Linux (or platform equivalent).
fn keymap_config_path() -> Result<PathBuf, KeymapConfigError> {
    reovim_arch::dirs::config_dir()
        .map(|d| d.join("reovim").join("keymap.toml"))
        .ok_or(KeymapConfigError::NoConfigDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Basic parsing tests
    // ========================================================================

    #[test]
    fn test_parse_empty_config() {
        let content = "";
        let config: KeymapConfig = toml::from_str(content).unwrap();
        assert!(config.is_empty());
        assert!(config.bindings.is_empty());
        assert!(config.remove.is_empty());
        assert!(config.resolvers.is_empty());
    }

    #[test]
    fn test_parse_bindings_section() {
        let content = r#"
[bindings.normal]
"j" = "cursor:down"
"k" = "cursor:up"
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();

        assert!(config.has_bindings());
        let normal = config.bindings_for_mode("normal").unwrap();
        assert_eq!(normal.bindings.len(), 2);
        assert_eq!(normal.bindings.get("j").unwrap(), "cursor:down");
        assert_eq!(normal.bindings.get("k").unwrap(), "cursor:up");
    }

    #[test]
    fn test_parse_multiple_modes() {
        let content = r#"
[bindings.normal]
"<C-s>" = "buffer:save"

[bindings.insert]
"jk" = "mode:normal"
"<C-c>" = "mode:normal"
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();

        assert!(config.has_bindings());

        let normal = config.bindings_for_mode("normal").unwrap();
        assert_eq!(normal.bindings.len(), 1);
        assert_eq!(normal.bindings.get("<C-s>").unwrap(), "buffer:save");

        let insert = config.bindings_for_mode("insert").unwrap();
        assert_eq!(insert.bindings.len(), 2);
        assert_eq!(insert.bindings.get("jk").unwrap(), "mode:normal");
    }

    #[test]
    fn test_parse_remove_section() {
        let content = r#"
[remove.normal]
keys = ["Q", "gQ", "ZZ"]
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();

        assert!(config.has_removals());
        let removals = config.removals_for_mode("normal").unwrap();
        assert_eq!(removals.keys.len(), 3);
        assert!(removals.keys.contains(&"Q".to_string()));
        assert!(removals.keys.contains(&"gQ".to_string()));
        assert!(removals.keys.contains(&"ZZ".to_string()));
    }

    #[test]
    fn test_parse_resolvers_section() {
        let content = r#"
[resolvers]
normal = "vim"
insert = "vim"
game = "eager"
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();

        assert!(config.has_resolvers());
        assert_eq!(config.resolver_for_mode("normal"), Some("vim"));
        assert_eq!(config.resolver_for_mode("insert"), Some("vim"));
        assert_eq!(config.resolver_for_mode("game"), Some("eager"));
        assert_eq!(config.resolver_for_mode("visual"), None);
    }

    #[test]
    fn test_parse_combined_config() {
        let content = r#"
[bindings.normal]
"<C-s>" = "buffer:save"
"<Leader>ff" = "picker:files"

[bindings.insert]
"jk" = "mode:normal"

[remove.normal]
keys = ["Q"]

[remove.insert]
keys = ["<C-a>"]

[resolvers]
normal = "vim"
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();

        // Bindings
        assert!(config.has_bindings());
        assert_eq!(config.bindings_for_mode("normal").unwrap().bindings.len(), 2);
        assert_eq!(config.bindings_for_mode("insert").unwrap().bindings.len(), 1);

        // Removals
        assert!(config.has_removals());
        assert_eq!(config.removals_for_mode("normal").unwrap().keys.len(), 1);
        assert_eq!(config.removals_for_mode("insert").unwrap().keys.len(), 1);

        // Resolvers
        assert!(config.has_resolvers());
        assert_eq!(config.resolver_for_mode("normal"), Some("vim"));
    }

    #[test]
    fn test_invalid_toml_error() {
        let content = "this is not valid toml [[[";
        let result: Result<KeymapConfig, _> = toml::from_str(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_file_returns_default() {
        let result = KeymapConfig::load_from(Path::new("/nonexistent/keymap.toml"));
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.is_empty());
    }

    // ========================================================================
    // Helper method tests
    // ========================================================================

    #[test]
    fn test_is_empty() {
        let empty: KeymapConfig = toml::from_str("").unwrap();
        assert!(empty.is_empty());

        let with_bindings: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"j" = "cursor:down"
"#,
        )
        .unwrap();
        assert!(!with_bindings.is_empty());
    }

    #[test]
    fn test_binding_modes_iterator() {
        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"j" = "cursor:down"

[bindings.insert]
"jk" = "mode:normal"

[bindings.visual]
# Empty section
"#,
        )
        .unwrap();

        let modes: Vec<&str> = config.binding_modes().collect();
        assert_eq!(modes.len(), 2);
        assert!(modes.contains(&"normal"));
        assert!(modes.contains(&"insert"));
        // visual is empty, so not included
    }

    #[test]
    fn test_removal_modes_iterator() {
        let config: KeymapConfig = toml::from_str(
            r#"
[remove.normal]
keys = ["Q"]

[remove.insert]
keys = []
"#,
        )
        .unwrap();

        let modes: Vec<&str> = config.removal_modes().collect();
        assert_eq!(modes.len(), 1);
        assert!(modes.contains(&"normal"));
        // insert has empty keys, so not included
    }

    // ========================================================================
    // Error type tests
    // ========================================================================

    #[test]
    fn test_error_display_no_config_dir() {
        let err = KeymapConfigError::NoConfigDir;
        assert_eq!(err.to_string(), "cannot determine config directory");
    }

    #[test]
    fn test_error_display_io() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let err = KeymapConfigError::Io(io_err);
        assert!(err.to_string().contains("IO error"));
        assert!(err.to_string().contains("access denied"));
    }

    #[test]
    fn test_error_display_parse() {
        let err = KeymapConfigError::Parse {
            message: "expected '='".into(),
            line: Some(5),
            column: Some(10),
        };
        let display = err.to_string();
        assert!(display.contains("parse error"));
        assert!(display.contains("expected '='"));
        assert!(display.contains("line 5"));
        assert!(display.contains("column 10"));
    }

    #[test]
    fn test_error_display_invalid_key_sequence() {
        let err = KeymapConfigError::InvalidKeySequence {
            keys: "<Invalid>".into(),
            mode: "normal".into(),
        };
        assert!(err.to_string().contains("invalid key sequence"));
        assert!(err.to_string().contains("<Invalid>"));
        assert!(err.to_string().contains("normal"));
    }

    #[test]
    fn test_error_display_invalid_command() {
        let err = KeymapConfigError::InvalidCommand {
            command: "bad::command".into(),
            mode: "insert".into(),
        };
        assert!(err.to_string().contains("invalid command"));
        assert!(err.to_string().contains("bad::command"));
    }

    #[test]
    fn test_error_display_unknown_resolver() {
        let err = KeymapConfigError::UnknownResolver {
            name: "nonexistent".into(),
            mode: "normal".into(),
        };
        assert!(err.to_string().contains("unknown resolver"));
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: KeymapConfigError = io_err.into();
        assert!(matches!(err, KeymapConfigError::Io(_)));
    }

    #[test]
    fn test_error_from_toml() {
        let toml_err = toml::from_str::<KeymapConfig>("invalid [[[").unwrap_err();
        let err: KeymapConfigError = toml_err.into();
        assert!(matches!(err, KeymapConfigError::Parse { .. }));
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_special_key_sequences() {
        let content = r#"
[bindings.normal]
"<C-w>h" = "window:left"
"<C-w><C-h>" = "window:left"
"<A-Enter>" = "editor:newline"
"<S-Tab>" = "editor:dedent"
"<Esc>" = "mode:normal"
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();
        let normal = config.bindings_for_mode("normal").unwrap();

        assert_eq!(normal.bindings.get("<C-w>h").unwrap(), "window:left");
        assert_eq!(normal.bindings.get("<A-Enter>").unwrap(), "editor:newline");
        assert_eq!(normal.bindings.get("<S-Tab>").unwrap(), "editor:dedent");
        assert_eq!(normal.bindings.get("<Esc>").unwrap(), "mode:normal");
    }

    #[test]
    fn test_leader_key_sequences() {
        let content = r#"
[bindings.normal]
"<Leader>ff" = "picker:files"
"<Leader>fg" = "picker:grep"
"<Leader>fb" = "picker:buffers"
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();
        let normal = config.bindings_for_mode("normal").unwrap();

        assert_eq!(normal.bindings.get("<Leader>ff").unwrap(), "picker:files");
        assert_eq!(normal.bindings.get("<Leader>fg").unwrap(), "picker:grep");
    }

    #[test]
    fn test_empty_mode_bindings() {
        let content = r"
[bindings.normal]
# No bindings here
";
        let config: KeymapConfig = toml::from_str(content).unwrap();

        // Mode exists but has no bindings
        let normal = config.bindings_for_mode("normal");
        assert!(normal.is_some());
        assert!(normal.unwrap().bindings.is_empty());

        // has_bindings should return false
        assert!(!config.has_bindings());
    }

    #[test]
    fn test_command_id_formats() {
        let content = r#"
[bindings.normal]
"a" = "editor:append"
"b" = "motions:word-forward"
"c" = "operators:change"
"#;
        let config: KeymapConfig = toml::from_str(content).unwrap();
        let normal = config.bindings_for_mode("normal").unwrap();

        // All valid module:command format
        assert_eq!(normal.bindings.get("a").unwrap(), "editor:append");
        assert_eq!(normal.bindings.get("b").unwrap(), "motions:word-forward");
        assert_eq!(normal.bindings.get("c").unwrap(), "operators:change");
    }

    // ========================================================================
    // apply() tests
    // ========================================================================

    #[test]
    fn test_apply_adds_user_bindings() {
        use crate::server::registry::KeymapRegistry;

        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"<C-s>" = "buffer:save"
"#,
        )
        .unwrap();

        let mut registry = KeymapRegistry::new();
        let stats = config.apply(&mut registry).unwrap();

        assert_eq!(stats.bindings_added, 1);
        assert_eq!(stats.bindings_removed, 0);

        // Verify binding was registered at User layer
        let mode = normalize_mode_id("normal");
        let keys = KeySequence::parse("<C-s>").unwrap();
        let binding = registry.get_binding(&mode, &keys);
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().name(), "save");
    }

    #[test]
    fn test_user_layer_overrides_policy() {
        use {crate::server::registry::KeymapRegistry, reovim_kernel::api::v1::CommandId};

        let mut registry = KeymapRegistry::new();
        let mode = normalize_mode_id("normal");
        let keys = KeySequence::parse("j").unwrap();

        // Register at Policy layer (simulating Vim module)
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            CommandId::new(ModuleId::new("editor"), "cursor-down"),
        );

        // Apply user config that overrides j
        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"j" = "custom:scroll-down"
"#,
        )
        .unwrap();

        config.apply(&mut registry).unwrap();

        // User binding should win
        let binding = registry.get_binding(&mode, &keys);
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().name(), "scroll-down");
    }

    #[test]
    fn test_remove_disables_binding() {
        use {crate::server::registry::KeymapRegistry, reovim_kernel::api::v1::CommandId};

        let mut registry = KeymapRegistry::new();
        let mode = normalize_mode_id("normal");
        let keys = KeySequence::parse("Q").unwrap();

        // Register at Policy layer
        registry.register_at_layer(
            BindingLayer::Policy,
            &mode,
            keys.clone(),
            CommandId::new(ModuleId::new("editor"), "ex-mode"),
        );

        // Verify it exists
        assert!(registry.get_binding(&mode, &keys).is_some());

        // Apply removal
        let config: KeymapConfig = toml::from_str(
            r#"
[remove.normal]
keys = ["Q"]
"#,
        )
        .unwrap();

        let stats = config.apply(&mut registry).unwrap();

        assert_eq!(stats.bindings_removed, 1);
        // Binding should now be None (removed)
        assert!(registry.get_binding(&mode, &keys).is_none());
    }

    #[test]
    fn test_apply_invalid_keys_error() {
        use crate::server::registry::KeymapRegistry;

        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"<InvalidKey>" = "test:command"
"#,
        )
        .unwrap();

        let mut registry = KeymapRegistry::new();
        let result = config.apply(&mut registry);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KeymapConfigError::InvalidKeySequence { .. }));
    }

    #[test]
    fn test_apply_invalid_command_error() {
        use crate::server::registry::KeymapRegistry;

        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"j" = "invalid-no-colon"
"#,
        )
        .unwrap();

        let mut registry = KeymapRegistry::new();
        let result = config.apply(&mut registry);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KeymapConfigError::InvalidCommand { .. }));
    }

    #[test]
    fn test_apply_multiple_modes() {
        use crate::server::registry::KeymapRegistry;

        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"<C-s>" = "buffer:save"

[bindings.insert]
"jk" = "mode:normal"

[remove.normal]
keys = ["Q"]
"#,
        )
        .unwrap();

        let mut registry = KeymapRegistry::new();
        let stats = config.apply(&mut registry).unwrap();

        assert_eq!(stats.bindings_added, 2);
        assert_eq!(stats.bindings_removed, 1);

        // Verify normal mode binding
        let normal = normalize_mode_id("normal");
        let ctrl_s = KeySequence::parse("<C-s>").unwrap();
        assert!(registry.get_binding(&normal, &ctrl_s).is_some());

        // Verify insert mode binding
        let insert = normalize_mode_id("insert");
        let jk = KeySequence::parse("jk").unwrap();
        assert!(registry.get_binding(&insert, &jk).is_some());
    }

    #[test]
    fn test_normalize_mode_id_short_form() {
        let mode = normalize_mode_id("normal");
        assert_eq!(mode.module().as_str(), "editor");
        assert_eq!(mode.name(), "normal");
    }

    #[test]
    fn test_normalize_mode_id_full_form() {
        let mode = normalize_mode_id("vim:special");
        assert_eq!(mode.module().as_str(), "vim");
        assert_eq!(mode.name(), "special");
    }

    #[test]
    fn test_parse_command_id_valid() {
        let cmd = parse_command_id("editor:cursor-down").unwrap();
        assert_eq!(cmd.module().as_str(), "editor");
        assert_eq!(cmd.name(), "cursor-down");
    }

    #[test]
    fn test_parse_command_id_invalid() {
        assert!(parse_command_id("no-colon").is_none());
        assert!(parse_command_id("").is_none());
    }

    // ========================================================================
    // validate() tests
    // ========================================================================

    #[test]
    fn test_validate_valid_config() {
        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"j" = "editor:cursor-down"
"<C-s>" = "buffer:save"

[remove.normal]
keys = ["Q"]
"#,
        )
        .unwrap();

        let errors = config.validate();
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_validate_invalid_key_sequence() {
        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"<InvalidKey>" = "editor:test"
"#,
        )
        .unwrap();

        let errors = config.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            KeymapConfigError::InvalidKeySequence { keys, mode }
            if keys == "<InvalidKey>" && mode == "normal"
        ));
    }

    #[test]
    fn test_validate_invalid_command() {
        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"j" = "invalid-no-colon"
"#,
        )
        .unwrap();

        let errors = config.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            KeymapConfigError::InvalidCommand { command, mode }
            if command == "invalid-no-colon" && mode == "normal"
        ));
    }

    #[test]
    fn test_validate_invalid_remove_key() {
        let config: KeymapConfig = toml::from_str(
            r#"
[remove.normal]
keys = ["<BadKey>"]
"#,
        )
        .unwrap();

        let errors = config.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            KeymapConfigError::InvalidKeySequence { keys, mode }
            if keys == "<BadKey>" && mode == "normal"
        ));
    }

    #[test]
    fn test_validate_multiple_errors() {
        let config: KeymapConfig = toml::from_str(
            r#"
[bindings.normal]
"<BadKey1>" = "editor:test"
"j" = "no-colon"

[bindings.insert]
"<BadKey2>" = "also:bad:command"

[remove.normal]
keys = ["<BadRemove>"]
"#,
        )
        .unwrap();

        let errors = config.validate();
        // Should find: <BadKey1> invalid, no-colon invalid, <BadKey2> invalid, <BadRemove> invalid
        // Note: "also:bad:command" is valid format (module:command), just command name has colons
        assert!(errors.len() >= 3, "Expected at least 3 errors, got: {errors:?}");
    }

    #[test]
    fn test_validate_empty_config() {
        let config: KeymapConfig = toml::from_str("").unwrap();
        let errors = config.validate();
        assert!(errors.is_empty());
    }
}
