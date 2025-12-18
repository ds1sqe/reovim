//! Profile manager for loading, saving, and switching profiles

use {
    super::{
        ConfigError,
        loader::{file_exists, get_config_dir, get_profiles_dir, load_toml, save_toml},
        schema::{GlobalConfig, ProfileConfig},
    },
    std::path::PathBuf,
};

/// Manages configuration profiles
#[derive(Debug)]
pub struct ProfileManager {
    /// Base config directory (~/.config/reovim)
    config_dir: PathBuf,
    /// Profiles directory (~/.config/reovim/profiles)
    profiles_dir: PathBuf,
    /// Global configuration
    global_config: GlobalConfig,
    /// Currently loaded profile
    current_profile: Option<ProfileConfig>,
    /// Name of current profile
    current_name: Option<String>,
}

impl ProfileManager {
    /// Create a new profile manager, loading global config.
    ///
    /// Creates config directories if they don't exist.
    ///
    /// # Errors
    ///
    /// Returns error if config directories cannot be created or accessed.
    pub fn new() -> Result<Self, ConfigError> {
        let config_dir = get_config_dir()?;
        let profiles_dir = get_profiles_dir()?;

        let global_config_path = config_dir.join("config.toml");
        let global_config = if file_exists(&global_config_path) {
            load_toml(&global_config_path)?
        } else {
            GlobalConfig::default()
        };

        Ok(Self {
            config_dir,
            profiles_dir,
            global_config,
            current_profile: None,
            current_name: None,
        })
    }

    /// Create an empty profile manager (for fallback when init fails).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            config_dir: PathBuf::new(),
            profiles_dir: PathBuf::new(),
            global_config: GlobalConfig::default(),
            current_profile: None,
            current_name: None,
        }
    }

    /// Get the default profile name from global config.
    #[must_use]
    pub fn default_profile_name(&self) -> &str {
        &self.global_config.default_profile
    }

    /// Get the currently loaded profile name.
    #[must_use]
    pub fn current_name(&self) -> Option<&str> {
        self.current_name.as_deref()
    }

    /// Get the currently loaded profile.
    #[must_use]
    pub const fn current_profile(&self) -> Option<&ProfileConfig> {
        self.current_profile.as_ref()
    }

    /// Set the current profile (for live updates from settings menu).
    pub fn set_current_profile(&mut self, profile: ProfileConfig) {
        self.current_profile = Some(profile);
    }

    /// List all available profile names.
    #[must_use]
    pub fn list_profiles(&self) -> Vec<String> {
        let mut profiles = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.profiles_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "toml")
                    && let Some(stem) = path.file_stem()
                    && let Some(name) = stem.to_str()
                {
                    profiles.push(name.to_string());
                }
            }
        }

        profiles.sort();
        profiles
    }

    /// Load a profile by name.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::NotFound` if profile doesn't exist,
    /// or parse error if TOML is invalid.
    pub fn load_profile(&mut self, name: &str) -> Result<ProfileConfig, ConfigError> {
        let path = self.profile_path(name);

        if !file_exists(&path) {
            return Err(ConfigError::NotFound(name.to_string()));
        }

        let config: ProfileConfig = load_toml(&path)?;
        self.current_profile = Some(config.clone());
        self.current_name = Some(name.to_string());

        tracing::info!(profile = %name, "Profile loaded");
        Ok(config)
    }

    /// Load a profile by name without updating internal state.
    ///
    /// Use this for read-only operations like previews.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::NotFound` if profile doesn't exist,
    /// or parse error if TOML is invalid.
    pub fn load_profile_readonly(&self, name: &str) -> Result<ProfileConfig, ConfigError> {
        let path = self.profile_path(name);

        if !file_exists(&path) {
            return Err(ConfigError::NotFound(name.to_string()));
        }

        load_toml(&path)
    }

    /// Save a profile configuration.
    ///
    /// # Errors
    ///
    /// Returns error if serialization or file write fails.
    pub fn save_profile(&self, name: &str, config: &ProfileConfig) -> Result<(), ConfigError> {
        let path = self.profile_path(name);
        save_toml(&path, config)?;

        tracing::info!(profile = %name, path = %path.display(), "Profile saved");
        Ok(())
    }

    /// Delete a profile by name.
    ///
    /// # Errors
    ///
    /// Returns error if profile doesn't exist or deletion fails.
    pub fn delete_profile(&self, name: &str) -> Result<(), ConfigError> {
        let path = self.profile_path(name);

        if !file_exists(&path) {
            return Err(ConfigError::NotFound(name.to_string()));
        }

        std::fs::remove_file(&path)?;
        tracing::info!(profile = %name, "Profile deleted");
        Ok(())
    }

    /// Set the default profile in global config.
    ///
    /// # Errors
    ///
    /// Returns error if global config cannot be saved.
    pub fn set_default_profile(&mut self, name: &str) -> Result<(), ConfigError> {
        self.global_config.default_profile = name.to_string();
        let path = self.config_dir.join("config.toml");
        save_toml(&path, &self.global_config)?;

        tracing::info!(profile = %name, "Default profile set");
        Ok(())
    }

    /// Check if a profile exists.
    #[must_use]
    pub fn profile_exists(&self, name: &str) -> bool {
        file_exists(&self.profile_path(name))
    }

    /// Get the file path for a profile.
    fn profile_path(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(format!("{name}.toml"))
    }

    /// Create the default profile if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns error if profile cannot be saved.
    pub fn ensure_default_profile(&self) -> Result<(), ConfigError> {
        let default_name = self.default_profile_name();

        if !self.profile_exists(default_name) {
            let mut config = ProfileConfig::default();
            config.profile.name = default_name.to_string();
            config.profile.description = "Default configuration".to_string();
            self.save_profile(default_name, &config)?;
            tracing::info!(profile = %default_name, "Created default profile");
        }

        Ok(())
    }

    /// Initialize the profile system, creating defaults if needed.
    ///
    /// # Errors
    ///
    /// Returns error if initialization fails.
    pub fn initialize(&self) -> Result<(), ConfigError> {
        // Ensure global config exists
        let global_path = self.config_dir.join("config.toml");
        if !file_exists(&global_path) {
            save_toml(&global_path, &self.global_config)?;
        }

        // Ensure default profile exists
        self.ensure_default_profile()?;

        Ok(())
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to initialize ProfileManager, using empty");
            Self::empty()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_manager_empty() {
        let manager = ProfileManager::empty();
        assert_eq!(manager.default_profile_name(), "default");
        assert!(manager.current_name().is_none());
        assert!(manager.list_profiles().is_empty());
    }

    #[test]
    fn test_profile_path() {
        let manager = ProfileManager::empty();
        // Empty manager has empty path, but we can test the format
        let path = manager.profile_path("test");
        assert!(path.to_string_lossy().ends_with("test.toml"));
    }
}
