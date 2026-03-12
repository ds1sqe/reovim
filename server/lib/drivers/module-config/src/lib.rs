#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Module configuration driver for user enable/disable and per-module settings.
//!
//! Parses `~/.config/reovim/modules.toml` to determine which server modules
//! and client extensions should be loaded, and provides per-module settings
//! via [`ModuleConfigStore`] in the service registry.
//!
//! # Architecture
//!
//! This is a **driver** (mechanism layer). It provides:
//! - TOML parsing for module configuration
//! - Enable/disable queries for modules and extensions
//! - Validation warnings for unknown module/extension IDs
//! - A shared `ModuleConfigStore` service for per-module settings
//!
//! Policy decisions (which modules to actually load) live in the bootstrap
//! (app layer). This driver only parses and exposes data.

use std::{collections::HashMap, fmt, path::Path};

use {reovim_kernel::api::v1::Service, serde::Deserialize};

// ============================================================================
// Config Types
// ============================================================================

/// Per-module configuration entry in `modules.toml`.
#[allow(clippy::derive_partial_eq_without_eq)] // toml::Value doesn't impl Eq
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModuleEntry {
    /// Whether the module is enabled (default: `true` if not specified).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Opaque per-module settings (passed to module via `ModuleConfigStore`).
    #[serde(default)]
    pub settings: Option<toml::Value>,
}

/// Per-extension configuration entry in `modules.toml`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ExtensionEntry {
    /// Whether the extension is enabled (default: `true` if not specified).
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Top-level `modules.toml` configuration.
///
/// Controls which server modules and client extensions are loaded.
/// A missing config file falls back to the "official" preset (all enabled).
#[allow(clippy::derive_partial_eq_without_eq)] // contains ModuleEntry with toml::Value
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModulesConfig {
    /// Base preset to extend (default: `"official"`).
    #[serde(default = "default_preset")]
    pub extends: String,
    /// Per-module overrides (keyed by module ID string).
    #[serde(default)]
    pub modules: HashMap<String, ModuleEntry>,
    /// Per-extension overrides (keyed by extension kind string).
    #[serde(default)]
    pub extensions: HashMap<String, ExtensionEntry>,
}

const fn default_true() -> bool {
    true
}

fn default_preset() -> String {
    "official".to_string()
}

// ============================================================================
// Error Types
// ============================================================================

/// Error parsing or loading module configuration.
#[derive(Debug)]
pub enum ModuleConfigError {
    /// TOML syntax or deserialization error.
    Parse(String),
    /// IO error reading config file.
    Io(std::io::Error),
}

impl fmt::Display for ModuleConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "module config parse error: {msg}"),
            Self::Io(err) => write!(f, "module config IO error: {err}"),
        }
    }
}

impl std::error::Error for ModuleConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Parse(_) => None,
        }
    }
}

/// Error extracting a typed field from module config settings.
///
/// Returned when a field exists but has the wrong TOML type.
/// Contains verbose diagnostics for user-facing error messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFieldError {
    /// The module ID whose config was queried.
    pub module_id: String,
    /// The field name that was queried.
    pub field: String,
    /// The expected TOML type (e.g. `"bool"`, `"integer"`, `"string"`).
    pub expected: &'static str,
    /// The actual TOML value as a string.
    pub actual: String,
}

impl fmt::Display for ConfigFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "module '{}': field '{}' expected {}, got {}",
            self.module_id, self.field, self.expected, self.actual
        )
    }
}

impl std::error::Error for ConfigFieldError {}

/// Non-fatal warning from config validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleConfigWarning {
    /// Config references a module ID not in the builtin registry.
    UnknownModule {
        /// The unknown module ID.
        id: String,
    },
    /// Config references an extension kind that is not known.
    UnknownExtension {
        /// The unknown extension kind.
        kind: String,
    },
    /// Disabling a module that is a required dependency of an enabled module.
    DependencyConflict {
        /// The disabled module ID.
        disabled: String,
        /// The enabled module that requires it.
        required_by: String,
    },
}

impl fmt::Display for ModuleConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownModule { id } => {
                write!(f, "unknown module '{id}' in config")
            }
            Self::UnknownExtension { kind } => {
                write!(f, "unknown extension '{kind}' in config")
            }
            Self::DependencyConflict {
                disabled,
                required_by,
            } => {
                write!(
                    f,
                    "disabled module '{disabled}' is required by enabled module '{required_by}'"
                )
            }
        }
    }
}

// ============================================================================
// ModulesConfig Implementation
// ============================================================================

impl ModulesConfig {
    /// Parse module configuration from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`ModuleConfigError::Parse`] if the TOML is invalid or
    /// cannot be deserialized into `ModulesConfig`.
    pub fn parse(toml_str: &str) -> Result<Self, ModuleConfigError> {
        toml::from_str(toml_str).map_err(|e| ModuleConfigError::Parse(e.to_string()))
    }

    /// Load module configuration from a file path.
    ///
    /// Returns the "official" preset if the file does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`ModuleConfigError::Io`] for IO errors other than not-found.
    /// Returns [`ModuleConfigError::Parse`] for invalid TOML.
    pub fn load(path: &Path) -> Result<Self, ModuleConfigError> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::parse(&contents),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::official()),
            Err(err) => Err(ModuleConfigError::Io(err)),
        }
    }

    /// Return the "official" preset: all modules and extensions enabled,
    /// no overrides.
    #[must_use]
    pub fn official() -> Self {
        Self {
            extends: "official".to_string(),
            modules: HashMap::new(),
            extensions: HashMap::new(),
        }
    }

    /// Check if a module is enabled.
    ///
    /// Modules not mentioned in config default to enabled (official preset
    /// behavior).
    #[must_use]
    pub fn is_module_enabled(&self, module_id: &str) -> bool {
        self.modules
            .get(module_id)
            .is_none_or(|entry| entry.enabled)
    }

    /// Check if an extension is enabled.
    ///
    /// Extensions not mentioned in config default to enabled.
    #[must_use]
    pub fn is_extension_enabled(&self, kind: &str) -> bool {
        self.extensions.get(kind).is_none_or(|entry| entry.enabled)
    }

    /// Get per-module settings (if any).
    #[must_use]
    pub fn module_settings(&self, module_id: &str) -> Option<&toml::Value> {
        self.modules
            .get(module_id)
            .and_then(|entry| entry.settings.as_ref())
    }

    /// List module IDs that are explicitly disabled.
    #[must_use]
    pub fn disabled_modules(&self) -> Vec<&str> {
        self.modules
            .iter()
            .filter(|(_, entry)| !entry.enabled)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Build a `ModuleConfigStore` from the per-module settings in this config.
    ///
    /// Extracts `settings` from each `ModuleEntry` that has one and packages
    /// them into a `ModuleConfigStore` ready for `ServiceRegistry` registration.
    #[must_use]
    pub fn build_config_store(&self) -> ModuleConfigStore {
        let configs = self
            .modules
            .iter()
            .filter_map(|(id, entry)| entry.settings.clone().map(|s| (id.clone(), s)))
            .collect();
        ModuleConfigStore::new(configs)
    }

    /// List extension kinds that are explicitly disabled.
    #[must_use]
    pub fn disabled_extensions(&self) -> Vec<&str> {
        self.extensions
            .iter()
            .filter(|(_, entry)| !entry.enabled)
            .map(|(kind, _)| kind.as_str())
            .collect()
    }

    /// Validate that all module IDs in config are known.
    ///
    /// Returns warnings for any module ID not in `known`.
    #[must_use]
    pub fn validate_modules(&self, known: &[&str]) -> Vec<ModuleConfigWarning> {
        self.modules
            .keys()
            .filter(|id| !known.contains(&id.as_str()))
            .map(|id| ModuleConfigWarning::UnknownModule { id: id.clone() })
            .collect()
    }

    /// Validate that all extension kinds in config are known.
    ///
    /// Returns warnings for any extension kind not in `known`.
    #[must_use]
    pub fn validate_extensions(&self, known: &[&str]) -> Vec<ModuleConfigWarning> {
        self.extensions
            .keys()
            .filter(|kind| !known.contains(&kind.as_str()))
            .map(|kind| ModuleConfigWarning::UnknownExtension { kind: kind.clone() })
            .collect()
    }
}

// ============================================================================
// ModuleConfigStore
// ============================================================================

/// Service providing per-module settings to modules during `init()`.
///
/// Registered in `ServiceRegistry` by bootstrap. Modules query their
/// config section via `get(module_id)`.
pub struct ModuleConfigStore {
    configs: HashMap<String, toml::Value>,
}

impl ModuleConfigStore {
    /// Create a new store from a map of module ID to settings.
    #[must_use]
    pub const fn new(configs: HashMap<String, toml::Value>) -> Self {
        Self { configs }
    }

    /// Get the settings for a module.
    #[must_use]
    pub fn get(&self, module_id: &str) -> Option<&toml::Value> {
        self.configs.get(module_id)
    }

    /// Check if the store has no settings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

impl ModuleConfigStore {
    /// Extract a boolean field from a module's settings.
    ///
    /// Returns `Ok(None)` if the module or field is absent.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigFieldError`] if the field exists but is not a bool.
    pub fn get_bool(&self, module_id: &str, field: &str) -> Result<Option<bool>, ConfigFieldError> {
        let Some(settings) = self.get(module_id) else {
            return Ok(None);
        };
        let Some(value) = settings.get(field) else {
            return Ok(None);
        };
        value.as_bool().map_or_else(
            || {
                Err(ConfigFieldError {
                    module_id: module_id.to_string(),
                    field: field.to_string(),
                    expected: "bool",
                    actual: format!("{value}"),
                })
            },
            |v| Ok(Some(v)),
        )
    }

    /// Extract an integer field from a module's settings.
    ///
    /// Returns `Ok(None)` if the module or field is absent.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigFieldError`] if the field exists but is not an integer.
    pub fn get_int(&self, module_id: &str, field: &str) -> Result<Option<i64>, ConfigFieldError> {
        let Some(settings) = self.get(module_id) else {
            return Ok(None);
        };
        let Some(value) = settings.get(field) else {
            return Ok(None);
        };
        value.as_integer().map_or_else(
            || {
                Err(ConfigFieldError {
                    module_id: module_id.to_string(),
                    field: field.to_string(),
                    expected: "integer",
                    actual: format!("{value}"),
                })
            },
            |v| Ok(Some(v)),
        )
    }

    /// Extract a string field from a module's settings.
    ///
    /// Returns `Ok(None)` if the module or field is absent.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigFieldError`] if the field exists but is not a string.
    pub fn get_str(
        &self,
        module_id: &str,
        field: &str,
    ) -> Result<Option<String>, ConfigFieldError> {
        let Some(settings) = self.get(module_id) else {
            return Ok(None);
        };
        let Some(value) = settings.get(field) else {
            return Ok(None);
        };
        value.as_str().map_or_else(
            || {
                Err(ConfigFieldError {
                    module_id: module_id.to_string(),
                    field: field.to_string(),
                    expected: "string",
                    actual: format!("{value}"),
                })
            },
            |v| Ok(Some(v.to_string())),
        )
    }
}

impl Service for ModuleConfigStore {}

impl fmt::Debug for ModuleConfigStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleConfigStore")
            .field("count", &self.configs.len())
            .finish()
    }
}

// ============================================================================
// Builtin Module Manifest (#620)
// ============================================================================

/// Entry in the builtin module manifest (`builtins.toml`).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BuiltinEntry {
    /// Module identifier (matches `Module::id()`).
    pub id: String,
    /// Initialization tier (1 = service, 2 = utility, 3 = policy, 4 = feature).
    pub tier: u8,
    /// Shared library name (without `lib` prefix or `.so`/`.dylib` suffix).
    pub library: String,
}

/// Parsed builtin module manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinManifest {
    /// Module entries in declaration order.
    pub modules: Vec<BuiltinEntry>,
}

/// Intermediate TOML structure for deserialization.
#[derive(Deserialize)]
struct BuiltinManifestToml {
    modules: Vec<BuiltinEntry>,
}

impl BuiltinManifest {
    /// Parse a builtin manifest from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`ModuleConfigError::Parse`] if the TOML is invalid.
    pub fn parse(toml_str: &str) -> Result<Self, ModuleConfigError> {
        let raw: BuiltinManifestToml =
            toml::from_str(toml_str).map_err(|e| ModuleConfigError::Parse(e.to_string()))?;
        Ok(Self {
            modules: raw.modules,
        })
    }

    /// Load a builtin manifest from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`ModuleConfigError::Io`] for IO errors.
    /// Returns [`ModuleConfigError::Parse`] for invalid TOML.
    pub fn load(path: &Path) -> Result<Self, ModuleConfigError> {
        let contents = std::fs::read_to_string(path).map_err(ModuleConfigError::Io)?;
        Self::parse(&contents)
    }

    /// Get module IDs in declaration order.
    #[must_use]
    pub fn module_ids(&self) -> Vec<&str> {
        self.modules.iter().map(|e| e.id.as_str()).collect()
    }

    /// Get module IDs filtered by a predicate.
    #[must_use]
    pub fn module_ids_filtered<F>(&self, predicate: F) -> Vec<&str>
    where
        F: Fn(&str) -> bool,
    {
        self.modules
            .iter()
            .filter(|e| predicate(&e.id))
            .map(|e| e.id.as_str())
            .collect()
    }

    /// Look up a module entry by ID.
    #[must_use]
    pub fn get(&self, module_id: &str) -> Option<&BuiltinEntry> {
        self.modules.iter().find(|e| e.id == module_id)
    }

    /// Get the library name for a module.
    #[must_use]
    pub fn library_name(&self, module_id: &str) -> Option<&str> {
        self.get(module_id).map(|e| e.library.as_str())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
