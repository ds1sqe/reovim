//! Server discovery for reo-cli
//!
//! Scans port files to find running reovim server instances.

use std::path::PathBuf;

/// Information about a running reovim server instance
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// Process ID of the server
    pub pid: u32,
    /// TCP port the server is listening on
    pub port: u16,
}

/// Get the XDG data directory for reovim
fn data_dir() -> PathBuf {
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

/// Get the servers directory for port files
fn servers_dir() -> PathBuf {
    data_dir().join("servers")
}

/// Check if a process with the given PID exists
fn process_exists(pid: u32) -> bool {
    // Check if /proc/<pid> directory exists (Linux-specific but safe)
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// List running reovim server instances by scanning port files
///
/// Validates that the PID is still running and cleans up stale files.
pub fn list_servers() -> Vec<ServerInfo> {
    let servers_dir = servers_dir();
    let Ok(entries) = servers_dir.read_dir() else {
        return vec![];
    };

    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let filename = entry.file_name();
            let name = filename.to_str()?;

            // Parse PID from filename (e.g., "12345.port")
            let pid: u32 = name.strip_suffix(".port")?.parse().ok()?;

            // Check if process is still running
            if !process_exists(pid) {
                // Cleanup stale file
                let _ = std::fs::remove_file(entry.path());
                return None;
            }

            // Read port from file
            let port: u16 = std::fs::read_to_string(entry.path())
                .ok()?
                .trim()
                .parse()
                .ok()?;

            Some(ServerInfo { pid, port })
        })
        .collect()
}
