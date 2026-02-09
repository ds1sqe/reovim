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

    #[test]
    fn test_registry_default() {
        let registry = InstanceRegistry::default();
        let default_dir = InstanceRegistry::default_registry_dir();
        assert_eq!(registry.registry_dir(), &default_dir);
    }

    #[test]
    fn test_registry_unregister_nonexistent() {
        let (registry, _temp) = test_registry();
        // Unregistering a non-existent instance should succeed silently
        let result = registry.unregister("does-not-exist");
        assert!(result.is_ok());
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let (registry, _temp) = test_registry();
        let result = registry.get("does-not-exist").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_registry_list_nonexistent_dir() {
        // Create a registry pointing to a non-existent directory
        let registry = InstanceRegistry::with_dir(std::path::PathBuf::from(
            "/tmp/reovim-test-nonexistent-dir-12345",
        ));
        let instances = registry.list().unwrap();
        assert!(instances.is_empty());
    }

    #[test]
    fn test_registry_list_skips_non_json_files() {
        let (registry, _temp) = test_registry();

        std::fs::create_dir_all(registry.registry_dir()).unwrap();
        // Write a non-JSON file
        let path = registry.registry_dir().join("readme.txt");
        std::fs::write(&path, "not an instance").unwrap();

        // Should not include non-JSON files
        let instances = registry.list().unwrap();
        assert!(instances.is_empty());
        // Non-JSON file should not be deleted
        assert!(path.exists());
    }

    #[test]
    fn test_validate_name_exactly_63_chars() {
        let name = "a".repeat(63);
        assert!(InstanceRegistry::validate_name(&name).is_ok());
    }

    #[test]
    fn test_validate_name_exactly_64_chars() {
        let name = "a".repeat(64);
        assert!(InstanceRegistry::validate_name(&name).is_err());
    }

    #[test]
    fn test_validate_name_special_chars() {
        assert!(InstanceRegistry::validate_name("has.dot").is_err());
        assert!(InstanceRegistry::validate_name("has@at").is_err());
        assert!(InstanceRegistry::validate_name("has!bang").is_err());
    }

    #[test]
    fn test_validate_name_with_numbers_and_hyphens() {
        assert!(InstanceRegistry::validate_name("a-b-c").is_ok());
        assert!(InstanceRegistry::validate_name("a_b_c").is_ok());
        assert!(InstanceRegistry::validate_name("abc123").is_ok());
        assert!(InstanceRegistry::validate_name("1starts-with-number").is_ok());
    }

    #[test]
    fn test_registry_stale_entry_replaced() {
        let (registry, _temp) = test_registry();
        let pid = std::process::id();

        // Write a stale entry directly
        std::fs::create_dir_all(registry.registry_dir()).unwrap();
        let info = InstanceInfo::new(
            "stale-replace".to_string(),
            u32::MAX - 1, // fake PID
            TransportInfo::tcp("127.0.0.1", 12521),
        );
        let path = registry.registry_dir().join("stale-replace.json");
        let json = serde_json::to_string(&info).unwrap();
        std::fs::write(&path, json).unwrap();

        // Register with the same name should succeed (stale entry replaced)
        let new_info = InstanceInfo::new(
            "stale-replace".to_string(),
            pid,
            TransportInfo::tcp("127.0.0.1", 12522),
        );
        registry.register(&new_info).unwrap();

        // Should find the new entry
        let retrieved = registry.get("stale-replace").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().pid, pid);
    }

    #[test]
    fn test_registry_get_internal_stale_no_cleanup() {
        let (registry, _temp) = test_registry();

        // Write a stale entry
        std::fs::create_dir_all(registry.registry_dir()).unwrap();
        let info = InstanceInfo::new(
            "stale-no-clean".to_string(),
            u32::MAX - 1,
            TransportInfo::tcp("127.0.0.1", 12521),
        );
        let path = registry.registry_dir().join("stale-no-clean.json");
        let json = serde_json::to_string(&info).unwrap();
        std::fs::write(&path, json).unwrap();

        // get() returns None for stale entries but also cleans up
        let result = registry.get("stale-no-clean").unwrap();
        assert!(result.is_none());
        // File should be cleaned up by get() (cleanup_stale=true)
        assert!(!path.exists());
    }

    #[test]
    fn test_instance_path() {
        let (registry, _temp) = test_registry();
        let dir = registry.registry_dir().clone();
        let expected = dir.join("my-instance.json");
        // instance_path is private, but we can verify indirectly
        // by registering and checking the file exists
        let info = InstanceInfo::new(
            "my-instance".to_string(),
            std::process::id(),
            TransportInfo::tcp("127.0.0.1", 12521),
        );
        registry.register(&info).unwrap();
        assert!(expected.exists());
    }

    #[test]
    fn test_validate_name_path_traversal_with_backslash() {
        assert!(InstanceRegistry::validate_name("test\\escape").is_err());
    }

    #[test]
    fn test_validate_name_path_traversal_with_slash() {
        assert!(InstanceRegistry::validate_name("test/path").is_err());
    }

    #[test]
    fn test_validate_name_single_char() {
        assert!(InstanceRegistry::validate_name("a").is_ok());
        assert!(InstanceRegistry::validate_name("1").is_ok());
    }

    #[test]
    fn test_registry_list_stale_entries_removed() {
        let (registry, _temp) = test_registry();

        // Write stale entry directly (fake PID)
        std::fs::create_dir_all(registry.registry_dir()).unwrap();
        let info = InstanceInfo::new(
            "stale-list".to_string(),
            u32::MAX - 2,
            TransportInfo::tcp("127.0.0.1", 12521),
        );
        let path = registry.registry_dir().join("stale-list.json");
        let json = serde_json::to_string(&info).unwrap();
        std::fs::write(&path, json).unwrap();

        // List should filter out the stale entry and remove the file
        let instances = registry.list().unwrap();
        assert!(instances.is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn test_registry_register_then_get_returns_current_pid() {
        let (registry, _temp) = test_registry();
        let pid = std::process::id();

        let info =
            InstanceInfo::new("pid-check".to_string(), pid, TransportInfo::tcp("127.0.0.1", 12521));
        registry.register(&info).unwrap();

        let retrieved = registry.get("pid-check").unwrap().unwrap();
        assert_eq!(retrieved.pid, pid);
    }

    #[test]
    fn test_registry_list_multiple_live_and_stale() {
        let (registry, _temp) = test_registry();
        let pid = std::process::id();

        // Register a live instance
        let live = InstanceInfo::new(
            "live-instance".to_string(),
            pid,
            TransportInfo::tcp("127.0.0.1", 12521),
        );
        registry.register(&live).unwrap();

        // Write a stale entry
        let stale = InstanceInfo::new(
            "stale-instance".to_string(),
            u32::MAX - 3,
            TransportInfo::tcp("127.0.0.1", 12522),
        );
        let stale_path = registry.registry_dir().join("stale-instance.json");
        let json = serde_json::to_string(&stale).unwrap();
        std::fs::write(&stale_path, json).unwrap();

        // List should return only the live instance
        let instances = registry.list().unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].name, "live-instance");
        // Stale file should be removed
        assert!(!stale_path.exists());
    }

    #[test]
    fn test_validate_name_double_dots() {
        assert!(InstanceRegistry::validate_name("a..b").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_registry_list_skips_unreadable_files() {
        use std::os::unix::fs::PermissionsExt;

        let (registry, _temp) = test_registry();
        std::fs::create_dir_all(registry.registry_dir()).unwrap();

        // Write a valid JSON file, then make it unreadable
        let path = registry.registry_dir().join("unreadable.json");
        std::fs::write(&path, r#"{"name":"x","pid":1,"transport":"tcp"}"#).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

        // list() should skip the unreadable file without error
        let instances = registry.list().unwrap();
        assert!(instances.is_empty());

        // Restore permissions so TempDir cleanup works
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}
