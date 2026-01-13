//! Platform-specific directory paths.
//!
//! Linux equivalent: Platform-specific path resolution
//!
//! This module wraps the `dirs` crate to provide platform-agnostic
//! directory path resolution. The kernel uses these functions via
//! `reovim_arch::dirs::*` to maintain kernel purity.
//!
//! # Platform Behavior
//!
//! - **Linux/Unix**: Uses XDG Base Directory Specification
//!   - `data_local_dir`: `~/.local/share`
//!   - `cache_dir`: `~/.cache`
//!   - `config_dir`: `~/.config`
//!
//! - **macOS**: Uses Apple conventions
//!   - `data_local_dir`: `~/Library/Application Support`
//!   - `cache_dir`: `~/Library/Caches`
//!   - `config_dir`: `~/Library/Application Support`
//!
//! - **Windows**: Uses Known Folders
//!   - `data_local_dir`: `{FOLDERID_LocalAppData}`
//!   - `cache_dir`: `{FOLDERID_LocalAppData}`
//!   - `config_dir`: `{FOLDERID_RoamingAppData}`

use std::path::PathBuf;

/// Returns the path to the user's local data directory.
///
/// Platform-specific:
/// - Linux: `$XDG_DATA_HOME` or `~/.local/share`
/// - macOS: `~/Library/Application Support`
/// - Windows: `{FOLDERID_LocalAppData}` (e.g., `C:\Users\Alice\AppData\Local`)
#[must_use]
pub fn data_local_dir() -> Option<PathBuf> {
    dirs::data_local_dir()
}

/// Returns the path to the user's cache directory.
///
/// Platform-specific:
/// - Linux: `$XDG_CACHE_HOME` or `~/.cache`
/// - macOS: `~/Library/Caches`
/// - Windows: `{FOLDERID_LocalAppData}` (e.g., `C:\Users\Alice\AppData\Local`)
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir()
}

/// Returns the path to the user's config directory.
///
/// Platform-specific:
/// - Linux: `$XDG_CONFIG_HOME` or `~/.config`
/// - macOS: `~/Library/Application Support`
/// - Windows: `{FOLDERID_RoamingAppData}` (e.g., `C:\Users\Alice\AppData\Roaming`)
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir()
}

/// Returns the path to the user's home directory.
///
/// Platform-specific:
/// - Linux/macOS: `$HOME`
/// - Windows: `{FOLDERID_Profile}` (e.g., `C:\Users\Alice`)
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Returns the path to the user's state directory.
///
/// Platform-specific:
/// - Linux: `$XDG_STATE_HOME` or `~/.local/state`
/// - macOS: `~/Library/Application Support`
/// - Windows: `{FOLDERID_LocalAppData}` (e.g., `C:\Users\Alice\AppData\Local`)
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    dirs::state_dir()
}

/// Returns the path to the user's runtime directory.
///
/// Platform-specific:
/// - Linux: `$XDG_RUNTIME_DIR` (e.g., `/run/user/1000`)
/// - macOS: None
/// - Windows: None
#[must_use]
pub fn runtime_dir() -> Option<PathBuf> {
    dirs::runtime_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_local_dir_exists() {
        // Should return Some on all platforms
        let dir = data_local_dir();
        assert!(dir.is_some(), "data_local_dir should return Some");
    }

    #[test]
    fn test_cache_dir_exists() {
        let dir = cache_dir();
        assert!(dir.is_some(), "cache_dir should return Some");
    }

    #[test]
    fn test_config_dir_exists() {
        let dir = config_dir();
        assert!(dir.is_some(), "config_dir should return Some");
    }

    #[test]
    fn test_home_dir_exists() {
        let dir = home_dir();
        assert!(dir.is_some(), "home_dir should return Some");
    }
}
