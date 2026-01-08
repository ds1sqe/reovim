//! XDG directory helpers for reovim
//!
//! Centralizes path logic for data and config directories following
//! the XDG Base Directory specification.

use std::path::PathBuf;

/// Get the XDG data directory for reovim (~/.local/share/reovim)
#[must_use]
pub fn data_dir() -> PathBuf {
    // Follow XDG Base Directory spec
    // $XDG_DATA_HOME defaults to $HOME/.local/share
    std::env::var("XDG_DATA_HOME")
        .map_or_else(
            |_| {
                let home = std::env::var("HOME").expect("HOME environment variable not set");
                PathBuf::from(home).join(".local").join("share")
            },
            PathBuf::from,
        )
        .join("reovim")
}

/// Get the servers directory for port files (~/.local/share/reovim/servers)
#[must_use]
pub fn servers_dir() -> PathBuf {
    data_dir().join("servers")
}

/// Write a port file for the current process
///
/// Creates `~/.local/share/reovim/servers/<pid>.port` containing the port number.
///
/// # Errors
///
/// Returns an error if directory creation or file writing fails.
pub fn write_port_file(port: u16) -> std::io::Result<PathBuf> {
    let servers_dir = servers_dir();
    std::fs::create_dir_all(&servers_dir)?;

    let port_file = servers_dir.join(format!("{}.port", std::process::id()));
    std::fs::write(&port_file, port.to_string())?;
    tracing::info!("Port file written: {}", port_file.display());
    Ok(port_file)
}

/// Remove a port file
pub fn remove_port_file(path: &std::path::Path) {
    if let Err(e) = std::fs::remove_file(path) {
        tracing::warn!("Failed to remove port file: {}", e);
    }
}

/// RAII guard that removes the port file on drop
///
/// This ensures cleanup happens even on panic or signal termination.
pub struct PortFileGuard {
    path: PathBuf,
}

impl PortFileGuard {
    /// Create a new port file guard, writing the port file immediately
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation or file writing fails.
    pub fn new(port: u16) -> std::io::Result<Self> {
        let path = write_port_file(port)?;
        Ok(Self { path })
    }
}

impl Drop for PortFileGuard {
    fn drop(&mut self) {
        remove_port_file(&self.path);
    }
}

/// Clean up stale port files for processes that no longer exist
///
/// Scans the servers directory and removes port files for PIDs that are no longer running.
pub fn cleanup_stale_port_files() {
    let servers_dir = servers_dir();
    let Ok(entries) = std::fs::read_dir(&servers_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        let Some(pid_str) = name.strip_suffix(".port") else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        // Check if process exists (Linux: /proc/<pid>)
        if !std::path::Path::new(&format!("/proc/{pid}")).exists()
            && std::fs::remove_file(entry.path()).is_ok()
        {
            tracing::debug!("Removed stale port file for PID {}", pid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_contains_reovim() {
        let dir = data_dir();
        assert!(dir.ends_with("reovim"));
    }

    #[test]
    fn test_servers_dir_is_subdir() {
        let data = data_dir();
        let servers = servers_dir();
        assert!(servers.starts_with(&data));
        assert!(servers.ends_with("servers"));
    }
}
