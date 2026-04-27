//! File-based instance registry.
//!
//! Provides persistent storage for instance information, enabling discovery
//! of running reovim servers without requiring a central daemon.
//!
//! # Registry Location
//!
//! - **Unix**: `$XDG_RUNTIME_DIR/reovim/` or `/tmp/reovim-<user>/`
//! - **Windows**: `%LOCALAPPDATA%\reovim\run\`
//!
//! # File Format
//!
//! Each instance is stored as a JSON file named `<instance-name>.json`.

#[cfg(test)]
use super::info::TransportInfo;
use {
    super::info::InstanceInfo,
    reovim_arch::process_exists,
    std::{io, path::PathBuf},
};

/// File-based instance registry.
///
/// Stores instance information as JSON files in a platform-specific directory.
/// Automatically cleans up stale entries (dead PIDs) when listing or getting.
pub struct InstanceRegistry {
    registry_dir: PathBuf,
}

impl InstanceRegistry {
    /// Create a new registry using the default directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry_dir: Self::default_registry_dir(),
        }
    }

    /// Create a registry with a custom directory (for testing).
    #[must_use]
    pub const fn with_dir(registry_dir: PathBuf) -> Self {
        Self { registry_dir }
    }

    /// Get the default registry directory.
    #[must_use]
    pub fn default_registry_dir() -> PathBuf {
        #[cfg(unix)]
        {
            reovim_arch::dirs::runtime_dir().map_or_else(
                #[cfg_attr(coverage_nightly, coverage(off))]
                || {
                    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
                    PathBuf::from(format!("/tmp/reovim-{user}"))
                },
                |d| d.join("reovim"),
            )
        }

        #[cfg(windows)]
        {
            reovim_arch::dirs::data_local_dir().map_or_else(
                || PathBuf::from(r"C:\ProgramData\reovim\run"),
                |d| d.join("reovim").join("run"),
            )
        }
    }

    /// Get the registry directory path.
    #[must_use]
    pub const fn registry_dir(&self) -> &PathBuf {
        &self.registry_dir
    }

    /// Register an instance in the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The registry directory cannot be created
    /// - The file cannot be written
    /// - An instance with the same name already exists and is alive
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn register(&self, info: &InstanceInfo) -> io::Result<()> {
        // Validate instance name
        Self::validate_name(&info.name)?;

        // Ensure registry directory exists
        std::fs::create_dir_all(&self.registry_dir)?;

        let path = self.instance_path(&info.name);

        // Check if instance already exists and is alive
        if path.exists() {
            if let Ok(Some(existing)) = self.get_internal(&info.name, false)
                && process_exists(existing.pid)
            {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("Instance '{}' already exists (PID {})", info.name, existing.pid),
                ));
            }
            // Stale entry, remove it
            let _ = std::fs::remove_file(&path);
        }

        let json = serde_json::to_string_pretty(info)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Unregister an instance from the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be removed.
    pub fn unregister(&self, name: &str) -> io::Result<()> {
        let path = self.instance_path(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// List all registered instances.
    ///
    /// Automatically removes stale entries (dead PIDs).
    ///
    /// # Errors
    ///
    /// Returns an error if the registry directory cannot be read.
    pub fn list(&self) -> io::Result<Vec<InstanceInfo>> {
        let mut instances = Vec::new();

        if !self.registry_dir.exists() {
            return Ok(instances);
        }

        for entry in std::fs::read_dir(&self.registry_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip non-JSON files
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }

            let Ok(json) = std::fs::read_to_string(&path) else {
                continue;
            };

            let Ok(info) = serde_json::from_str::<InstanceInfo>(&json) else {
                // Malformed JSON, remove it
                let _ = std::fs::remove_file(&path);
                continue;
            };

            if process_exists(info.pid) {
                instances.push(info);
            } else {
                // Clean up stale entry
                let _ = std::fs::remove_file(&path);
            }
        }

        Ok(instances)
    }

    /// Get a specific instance by name.
    ///
    /// Returns `None` if the instance doesn't exist or is stale.
    /// Automatically removes stale entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn get(&self, name: &str) -> io::Result<Option<InstanceInfo>> {
        self.get_internal(name, true)
    }

    /// Internal get implementation with optional cleanup.
    fn get_internal(&self, name: &str, cleanup_stale: bool) -> io::Result<Option<InstanceInfo>> {
        let path = self.instance_path(name);

        if !path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&path)?;
        let info: InstanceInfo = serde_json::from_str(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if !process_exists(info.pid) {
            if cleanup_stale {
                // Clean up stale entry
                let _ = std::fs::remove_file(&path);
            }
            return Ok(None);
        }

        Ok(Some(info))
    }

    /// Validate an instance name.
    ///
    /// Valid names:
    /// - 1-63 characters
    /// - Start with alphanumeric
    /// - Contain only: a-z, A-Z, 0-9, -, _
    ///
    /// # Errors
    ///
    /// Returns an error if the name is invalid.
    pub fn validate_name(name: &str) -> io::Result<()> {
        if name.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Instance name cannot be empty",
            ));
        }

        if name.len() > 63 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Instance name cannot exceed 63 characters",
            ));
        }

        // SAFETY: We already checked name.is_empty() above, so first byte exists.
        // Using as_bytes()[0] is safe because all valid characters are ASCII.
        if !name.as_bytes()[0].is_ascii_alphanumeric() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Instance name must start with alphanumeric character",
            ));
        }

        for c in name.chars() {
            if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Instance name contains invalid character: '{c}'"),
                ));
            }
        }

        Ok(())
    }

    /// Get the file path for an instance.
    fn instance_path(&self, name: &str) -> PathBuf {
        self.registry_dir.join(format!("{name}.json"))
    }
}

impl Default for InstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
