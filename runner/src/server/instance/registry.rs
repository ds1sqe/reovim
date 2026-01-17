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

        // SAFETY: We already checked name.is_empty() above, so first char exists
        let Some(first) = name.chars().next() else {
            unreachable!("name is not empty, checked above");
        };
        if !first.is_ascii_alphanumeric() {
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

        // Check for path traversal attempts
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Instance name contains path traversal characters",
            ));
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
mod tests {
    use {super::*, tempfile::TempDir};

    fn test_registry() -> (InstanceRegistry, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let registry = InstanceRegistry::with_dir(temp_dir.path().to_path_buf());
        (registry, temp_dir)
    }

    #[test]
    fn test_registry_register_unregister() {
        let (registry, _temp) = test_registry();

        let info = InstanceInfo::new(
            "test-instance".to_string(),
            std::process::id(),
            TransportInfo::tcp("127.0.0.1", 12521),
        );

        // Register
        registry.register(&info).unwrap();

        // Verify exists
        let retrieved = registry.get("test-instance").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-instance");

        // Unregister
        registry.unregister("test-instance").unwrap();

        // Verify removed
        let retrieved = registry.get("test-instance").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_registry_list_instances() {
        let (registry, _temp) = test_registry();
        let pid = std::process::id();

        // Register multiple instances
        for i in 0u16..3u16 {
            let info = InstanceInfo::new(
                format!("instance-{i}"),
                pid,
                TransportInfo::tcp("127.0.0.1", 12521 + i),
            );
            registry.register(&info).unwrap();
        }

        // List all
        let instances = registry.list().unwrap();
        assert_eq!(instances.len(), 3);
    }

    #[test]
    fn test_registry_stale_cleanup() {
        let (registry, _temp) = test_registry();

        // Register with a fake PID that doesn't exist
        let info = InstanceInfo::new(
            "stale-instance".to_string(),
            u32::MAX - 1, // Very unlikely to exist
            TransportInfo::tcp("127.0.0.1", 12521),
        );

        // Write directly to bypass alive check
        std::fs::create_dir_all(registry.registry_dir()).unwrap();
        let path = registry.registry_dir().join("stale-instance.json");
        let json = serde_json::to_string(&info).unwrap();
        std::fs::write(&path, json).unwrap();

        // Get should return None and clean up
        let retrieved = registry.get("stale-instance").unwrap();
        assert!(retrieved.is_none());

        // File should be removed
        assert!(!path.exists());
    }

    #[test]
    fn test_registry_duplicate_name_prevented() {
        let (registry, _temp) = test_registry();
        let pid = std::process::id();

        let info =
            InstanceInfo::new("duplicate".to_string(), pid, TransportInfo::tcp("127.0.0.1", 12521));

        // First registration succeeds
        registry.register(&info).unwrap();

        // Second registration with same name fails
        let result = registry.register(&info);
        assert!(result.is_err());
        assert!(result.unwrap_err().kind() == io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn test_instance_name_validation() {
        // Valid names
        assert!(InstanceRegistry::validate_name("default").is_ok());
        assert!(InstanceRegistry::validate_name("my-project").is_ok());
        assert!(InstanceRegistry::validate_name("project_123").is_ok());
        assert!(InstanceRegistry::validate_name("A").is_ok());

        // Invalid names
        assert!(InstanceRegistry::validate_name("").is_err()); // Empty
        assert!(InstanceRegistry::validate_name("-invalid").is_err()); // Starts with hyphen
        assert!(InstanceRegistry::validate_name("_invalid").is_err()); // Starts with underscore
        assert!(InstanceRegistry::validate_name("has spaces").is_err()); // Contains space
        assert!(InstanceRegistry::validate_name("../etc/passwd").is_err()); // Path traversal
        assert!(InstanceRegistry::validate_name("a".repeat(64).as_str()).is_err()); // Too long
    }

    #[test]
    fn test_registry_empty_dir() {
        let (registry, _temp) = test_registry();

        // List on empty registry should return empty vec
        let instances = registry.list().unwrap();
        assert!(instances.is_empty());
    }

    #[test]
    fn test_registry_malformed_json() {
        let (registry, _temp) = test_registry();

        // Write malformed JSON
        std::fs::create_dir_all(registry.registry_dir()).unwrap();
        let path = registry.registry_dir().join("malformed.json");
        std::fs::write(&path, "not valid json {").unwrap();

        // List should handle gracefully and remove the file
        let instances = registry.list().unwrap();
        assert!(instances.is_empty());
        assert!(!path.exists());
    }
}
