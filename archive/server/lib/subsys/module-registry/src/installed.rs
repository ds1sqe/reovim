//! Installed module tracking — metadata about modules currently on disk.

use std::{collections::HashMap, fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::source::ModuleSource;

/// Metadata for a single installed third-party module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledModule {
    /// Module identifier (from `module.toml`).
    pub id: String,
    /// Installed version string.
    pub version: String,
    /// Where the module was sourced from.
    pub source: ModuleSource,
    /// Path to the installed module directory.
    pub install_path: PathBuf,
    /// Path to the compiled shared library (if built).
    #[serde(default)]
    pub library_path: Option<PathBuf>,
}

impl InstalledModule {
    /// Check if the module has been built (has a compiled library).
    #[must_use]
    pub const fn is_built(&self) -> bool {
        self.library_path.is_some()
    }
}

impl fmt::Display for InstalledModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} v{} ({})", self.id, self.version, self.source)
    }
}

/// Collection of installed modules, serialized to `installed.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledModules {
    /// Map of module ID to installed metadata.
    pub modules: HashMap<String, InstalledModule>,
}

impl InstalledModules {
    /// Create an empty installed modules collection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    /// Parse installed modules from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid.
    pub fn parse(json_str: &str) -> Result<Self, String> {
        serde_json::from_str(json_str).map_err(|e| format!("installed modules parse error: {e}"))
    }

    /// Load installed modules from a file path.
    ///
    /// Returns an empty collection if the file does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error for IO errors (other than not-found) or invalid JSON.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::parse(&contents),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::new()),
            Err(err) => Err(format!("installed modules IO error: {err}")),
        }
    }

    /// Save installed modules to a file path as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or IO fails.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("installed modules serialize error: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("installed modules write error: {e}"))
    }

    /// Get an installed module by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&InstalledModule> {
        self.modules.get(id)
    }

    /// Check if a module is installed.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.modules.contains_key(id)
    }

    /// Add or update an installed module entry.
    pub fn insert(&mut self, module: InstalledModule) {
        self.modules.insert(module.id.clone(), module);
    }

    /// Remove an installed module entry, returning it if present.
    pub fn remove(&mut self, id: &str) -> Option<InstalledModule> {
        self.modules.remove(id)
    }

    /// List all installed module IDs.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.modules.keys().map(String::as_str).collect()
    }

    /// Number of installed modules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if there are no installed modules.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}
