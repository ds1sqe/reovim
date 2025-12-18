//! Configuration file loading and saving utilities

use {
    serde::{Serialize, de::DeserializeOwned},
    std::{
        fmt, fs, io,
        path::{Path, PathBuf},
    },
};

/// Configuration-related errors
#[derive(Debug)]
pub enum ConfigError {
    /// IO error during file operations
    Io(io::Error),
    /// TOML parsing error
    Parse(toml::de::Error),
    /// TOML serialization error
    Serialize(toml::ser::Error),
    /// Profile not found
    NotFound(String),
    /// Missing HOME environment variable
    NoHome,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Parse(e) => write!(f, "TOML parse error: {e}"),
            Self::Serialize(e) => write!(f, "TOML serialize error: {e}"),
            Self::NotFound(name) => write!(f, "Profile not found: {name}"),
            Self::NoHome => write!(f, "HOME environment variable not set"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::Serialize(e) => Some(e),
            Self::NotFound(_) | Self::NoHome => None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        Self::Parse(err)
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(err: toml::ser::Error) -> Self {
        Self::Serialize(err)
    }
}

/// Get the reovim config directory, creating it if needed.
///
/// Uses XDG Base Directory specification:
/// - `$XDG_CONFIG_HOME/reovim` if set
/// - `$HOME/.config/reovim` otherwise
///
/// # Errors
///
/// Returns `ConfigError::NoHome` if HOME is not set,
/// or `ConfigError::Io` if directory creation fails.
pub fn get_config_dir() -> Result<PathBuf, ConfigError> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME")
                .map(|home| PathBuf::from(home).join(".config"))
                .map_err(|_| ConfigError::NoHome)
        })?;

    let reovim_config = config_home.join("reovim");
    fs::create_dir_all(&reovim_config)?;
    Ok(reovim_config)
}

/// Get the profiles directory, creating it if needed.
///
/// # Errors
///
/// Returns error if config directory cannot be accessed or created.
pub fn get_profiles_dir() -> Result<PathBuf, ConfigError> {
    let config_dir = get_config_dir()?;
    let profiles_dir = config_dir.join("profiles");
    fs::create_dir_all(&profiles_dir)?;
    Ok(profiles_dir)
}

/// Load and deserialize a TOML file.
///
/// # Errors
///
/// Returns error if file cannot be read or parsed.
pub fn load_toml<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    let content = fs::read_to_string(path)?;
    let value = toml::from_str(&content)?;
    Ok(value)
}

/// Serialize and save to a TOML file.
///
/// Creates parent directories if they don't exist.
///
/// # Errors
///
/// Returns error if serialization or file write fails.
pub fn save_toml<T: Serialize>(path: &Path, config: &T) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

/// Check if a file exists.
#[must_use]
pub fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

#[cfg(test)]
mod tests {
    use {super::*, crate::config::ProfileConfig};

    #[test]
    fn test_get_config_dir() {
        // This test depends on HOME being set
        if std::env::var("HOME").is_ok() {
            let dir = get_config_dir().unwrap();
            assert!(dir.ends_with("reovim"));
        }
    }

    #[test]
    fn test_load_save_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.toml");

        let config = ProfileConfig::default();
        save_toml(&path, &config).unwrap();

        let loaded: ProfileConfig = load_toml(&path).unwrap();
        assert_eq!(loaded.profile.version, config.profile.version);
    }
}
