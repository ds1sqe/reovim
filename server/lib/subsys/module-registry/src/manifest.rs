//! Module manifest (`module.toml`) parsing for third-party modules.
//!
//! Each third-party module includes a `module.toml` at its crate root
//! describing identity, capabilities, dependencies, and build info.

use std::fmt;

use serde::Deserialize;

// ============================================================================
// Manifest Types
// ============================================================================

/// Parsed `module.toml` manifest for a third-party module.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModuleManifest {
    /// Module identity section.
    pub module: ModuleInfo,
    /// Capability declarations.
    #[serde(default)]
    pub capabilities: CapabilitySection,
    /// Version constraints on other modules.
    #[serde(default)]
    pub dependencies: DependencySection,
    /// Build configuration.
    #[serde(default)]
    pub build: BuildSection,
}

/// `[module]` section — identity and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModuleInfo {
    /// Unique module identifier (e.g., `"my-module"`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Semantic version string (e.g., `"1.0.0"`).
    pub version: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Author list.
    #[serde(default)]
    pub authors: Vec<String>,
}

/// `[capabilities]` section — what the module provides and requires.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct CapabilitySection {
    /// Capabilities this module provides.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Capabilities this module requires.
    #[serde(default)]
    pub requires: Vec<String>,
}

/// `[dependencies]` section — version constraints on other modules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DependencySection {
    /// Map of module-id to version constraint string (e.g., `"^0.9.0"`).
    #[serde(flatten)]
    pub constraints: std::collections::HashMap<String, String>,
}

/// `[build]` section — how to compile the module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct BuildSection {
    /// Cargo crate name (defaults to module id if absent).
    #[serde(default, rename = "crate-name")]
    pub crate_name: Option<String>,
}

// ============================================================================
// Error Types
// ============================================================================

/// Error parsing a `module.toml` manifest.
#[derive(Debug)]
pub enum ManifestError {
    /// TOML syntax or deserialization error.
    Parse(String),
    /// IO error reading manifest file.
    Io(std::io::Error),
    /// Missing required field.
    MissingField(&'static str),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "manifest parse error: {msg}"),
            Self::Io(err) => write!(f, "manifest IO error: {err}"),
            Self::MissingField(field) => write!(f, "manifest missing required field: {field}"),
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Parse(_) | Self::MissingField(_) => None,
        }
    }
}

// ============================================================================
// ModuleManifest Implementation
// ============================================================================

impl ModuleManifest {
    /// Parse a manifest from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Parse`] if the TOML is invalid.
    pub fn parse(toml_str: &str) -> Result<Self, ManifestError> {
        toml::from_str(toml_str).map_err(|e| ManifestError::Parse(e.to_string()))
    }

    /// Load a manifest from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Io`] for IO errors.
    /// Returns [`ManifestError::Parse`] for invalid TOML.
    pub fn load(path: &std::path::Path) -> Result<Self, ManifestError> {
        let contents = std::fs::read_to_string(path).map_err(ManifestError::Io)?;
        Self::parse(&contents)
    }

    /// Get the module ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.module.id
    }

    /// Get the module version string.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.module.version
    }

    /// Get the crate name for building (falls back to module ID).
    #[must_use]
    pub fn crate_name(&self) -> &str {
        self.build.crate_name.as_deref().unwrap_or(&self.module.id)
    }

    /// Get the list of provided capabilities.
    #[must_use]
    pub fn provides(&self) -> &[String] {
        &self.capabilities.provides
    }

    /// Get the list of required capabilities.
    #[must_use]
    pub fn requires(&self) -> &[String] {
        &self.capabilities.requires
    }

    /// Get dependency version constraints.
    #[must_use]
    pub const fn dependency_constraints(&self) -> &std::collections::HashMap<String, String> {
        &self.dependencies.constraints
    }
}
