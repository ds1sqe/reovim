//! Lock file (`modules.lock`) for module integrity verification.
//!
//! Records module metadata and SHA-256 checksums for external modules.
//! Used to detect tampered or outdated module files.

use std::{
    io::{self, Read as _},
    path::Path,
};

use {
    reovim_kernel::api::v1::{API_VERSION_STR, ModuleId},
    serde::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
};

use super::registry::ModuleRegistry;

// ============================================================================
// Types
// ============================================================================

/// Complete lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesLock {
    /// Lock file metadata.
    pub meta: LockMeta,
    /// Module entries.
    #[serde(rename = "module", default)]
    pub modules: Vec<LockEntry>,
}

/// Lock file metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockMeta {
    /// Reovim version that generated this lock file.
    pub reovim_version: String,
    /// Kernel API version.
    pub api_version: String,
    /// ISO 8601 timestamp.
    pub generated: String,
}

/// A single module entry in the lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    /// Module ID.
    pub id: String,
    /// Module version.
    pub version: String,
    /// Source type (builtin or external).
    pub source: ModuleSource,
    /// File path (external modules only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// SHA-256 checksum (external modules only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// API version the module was built against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    /// Required dependencies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    /// Optional dependencies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_dependencies: Vec<String>,
}

/// Module source type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleSource {
    /// Compile-time linked module.
    Builtin,
    /// Runtime loaded `.so` module.
    External,
}

/// A checksum mismatch found during verification.
#[derive(Debug, Clone)]
pub struct ChecksumMismatch {
    /// Module ID.
    pub id: String,
    /// Expected checksum from lock file.
    pub expected: String,
    /// Actual checksum computed from file.
    pub actual: String,
    /// File path.
    pub path: String,
}

/// Lock file errors.
#[derive(Debug)]
pub enum LockFileError {
    /// I/O error reading/writing the file.
    Io(io::Error),
    /// TOML parsing error.
    Parse(String),
    /// TOML serialization error.
    Serialize(String),
}

impl std::fmt::Display for LockFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "lock file I/O error: {e}"),
            Self::Parse(e) => write!(f, "lock file parse error: {e}"),
            Self::Serialize(e) => write!(f, "lock file serialize error: {e}"),
        }
    }
}

impl std::error::Error for LockFileError {}

impl From<io::Error> for LockFileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

// ============================================================================
// SHA-256
// ============================================================================

/// Compute SHA-256 hex digest of a file.
///
/// # Errors
///
/// Returns `io::Error` if the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String, io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

// ============================================================================
// ModulesLock
// ============================================================================

impl ModulesLock {
    /// Generate a lock file from registry state.
    #[must_use]
    pub fn generate(registry: &ModuleRegistry, reovim_version: &str) -> Self {
        let ids = registry.registered_ids();
        let init_order = registry.init_order();

        // Use init order if available, otherwise use registration order
        let ordered_ids = if init_order.is_empty() {
            ids
        } else {
            init_order
        };

        let mut modules = Vec::new();
        for id in &ordered_ids {
            let path = registry.module_path(id);
            let source = if path.is_some() {
                ModuleSource::External
            } else {
                ModuleSource::Builtin
            };

            let sha256 = path.as_ref().and_then(|p| sha256_file(p).ok());

            let path_str = path.map(|p| p.display().to_string());

            modules.push(LockEntry {
                id: id.as_str().to_owned(),
                version: String::new(), // Filled from probe if available
                source,
                path: path_str,
                sha256,
                api_version: Some(API_VERSION_STR.to_owned()),
                dependencies: Vec::new(),
                optional_dependencies: Vec::new(),
            });
        }

        let now = chrono::Utc::now().to_rfc3339();

        Self {
            meta: LockMeta {
                reovim_version: reovim_version.to_owned(),
                api_version: API_VERSION_STR.to_owned(),
                generated: now,
            },
            modules,
        }
    }

    /// Load a lock file from disk.
    ///
    /// # Errors
    ///
    /// Returns `LockFileError` if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self, LockFileError> {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| LockFileError::Parse(e.to_string()))
    }

    /// Save the lock file to disk.
    ///
    /// # Errors
    ///
    /// Returns `LockFileError` if serialization or writing fails.
    pub fn save(&self, path: &Path) -> Result<(), LockFileError> {
        let header = "# Generated by reovim. DO NOT EDIT.\n\n";
        let body =
            toml::to_string_pretty(self).map_err(|e| LockFileError::Serialize(e.to_string()))?;

        let content = format!("{header}{body}");
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Verify SHA-256 checksums for all external modules.
    #[must_use]
    pub fn verify_checksums(&self) -> Vec<ChecksumMismatch> {
        let mut mismatches = Vec::new();

        for entry in &self.modules {
            if entry.source != ModuleSource::External {
                continue;
            }

            let (Some(expected), Some(path_str)) = (&entry.sha256, &entry.path) else {
                continue;
            };

            let path = Path::new(path_str);
            if !path.exists() {
                mismatches.push(ChecksumMismatch {
                    id: entry.id.clone(),
                    expected: expected.clone(),
                    actual: "<file not found>".into(),
                    path: path_str.clone(),
                });
                continue;
            }

            match sha256_file(path) {
                Ok(actual) if actual != *expected => {
                    mismatches.push(ChecksumMismatch {
                        id: entry.id.clone(),
                        expected: expected.clone(),
                        actual,
                        path: path_str.clone(),
                    });
                }
                Err(_) => {
                    mismatches.push(ChecksumMismatch {
                        id: entry.id.clone(),
                        expected: expected.clone(),
                        actual: "<read error>".into(),
                        path: path_str.clone(),
                    });
                }
                Ok(_) => {} // Match - no mismatch
            }
        }

        mismatches
    }

    /// Check if this lock file is stale (generated by different version).
    #[must_use]
    pub fn is_stale(&self, current_version: &str) -> bool {
        self.meta.reovim_version != current_version
    }

    /// Find an entry by module ID.
    #[must_use]
    pub fn find(&self, id: &ModuleId) -> Option<&LockEntry> {
        self.modules.iter().find(|e| e.id == id.as_str())
    }
}
