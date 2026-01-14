//! Server discovery for finding running reovim instances.
//!
//! Scans default ports (12521-12530) to find running servers.

use std::{net::TcpStream, time::Duration};

/// Default server port.
pub const DEFAULT_PORT: u16 = 12521;

/// Number of ports to scan for fallback.
pub const PORT_FALLBACK_COUNT: u16 = 10;

/// Default host for local connections.
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// Information about a discovered server.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// Process ID (if available).
    pub pid: Option<u32>,
    /// Port the server is listening on.
    pub port: u16,
    /// Host address.
    pub host: String,
}

impl ServerInfo {
    /// Create server info for a port.
    #[must_use]
    pub fn new(port: u16) -> Self {
        Self {
            pid: None,
            port,
            host: DEFAULT_HOST.to_string(),
        }
    }

    /// Create with PID.
    #[must_use]
    pub const fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }
}

/// List running reovim servers by scanning ports.
///
/// Checks ports 12521-12530 for listening servers.
#[must_use]
pub fn list_servers() -> Vec<ServerInfo> {
    let mut servers = Vec::new();

    for port in DEFAULT_PORT..DEFAULT_PORT + PORT_FALLBACK_COUNT {
        if is_port_open(DEFAULT_HOST, port) {
            let mut info = ServerInfo::new(port);

            // Try to get PID from /proc on Linux
            #[cfg(target_os = "linux")]
            if let Some(pid) = find_pid_for_port(port) {
                info = info.with_pid(pid);
            }

            servers.push(info);
        }
    }

    servers
}

/// Check if a port is open (has a listening server).
fn is_port_open(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| {
            // Fallback to parsing as socket addr
            std::net::SocketAddr::from(([127, 0, 0, 1], port))
        }),
        Duration::from_millis(100),
    )
    .is_ok()
}

/// Find PID for a port by scanning /proc/net/tcp on Linux.
#[cfg(target_os = "linux")]
fn find_pid_for_port(port: u16) -> Option<u32> {
    use std::fs;

    // Read /proc/net/tcp to find the inode for this port
    let tcp_content = fs::read_to_string("/proc/net/tcp").ok()?;

    // Port in /proc/net/tcp is in hex, little-endian for local address
    let port_hex = format!("{port:04X}");

    let mut target_inode: Option<u64> = None;

    for line in tcp_content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        // local_address is parts[1], format: "IP:PORT"
        let local_addr = parts[1];
        if let Some(local_port) = local_addr.split(':').nth(1)
            && local_port == port_hex
        {
            // Found the port, get inode (parts[9])
            if let Ok(inode) = parts[9].parse::<u64>() {
                target_inode = Some(inode);
                break;
            }
        }
    }

    let inode = target_inode?;

    // Now scan /proc/*/fd/* to find which process has this inode
    let proc_dir = fs::read_dir("/proc").ok()?;

    for entry in proc_dir.flatten() {
        let pid_str = entry.file_name().to_string_lossy().to_string();
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let Ok(fd_dir) = fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };

        for fd_entry in fd_dir.flatten() {
            let Ok(link) = fs::read_link(fd_entry.path()) else {
                continue;
            };

            let link_str = link.to_string_lossy();
            if link_str.contains(&format!("socket:[{inode}]")) {
                return Some(pid);
            }
        }
    }

    None
}

/// Auto-discover a server and return connection config.
///
/// # Errors
///
/// Returns error if no servers found or multiple servers require explicit selection.
pub fn auto_discover() -> Result<super::ConnectionConfig, String> {
    let servers = list_servers();

    match servers.len() {
        0 => Err("No running reovim servers found".to_string()),
        1 => Ok(super::ConnectionConfig::tcp(&servers[0].host, servers[0].port)),
        _ => {
            let list = servers
                .iter()
                .map(|s| {
                    s.pid.map_or_else(
                        || format!("  --tcp {}:{}", s.host, s.port),
                        |pid| format!("  --tcp {}:{} (PID {pid})", s.host, s.port),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Err(format!("Multiple servers running. Use --tcp to specify:\n{list}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info_new() {
        let info = ServerInfo::new(12521);
        assert_eq!(info.port, 12521);
        assert_eq!(info.host, "127.0.0.1");
        assert!(info.pid.is_none());
    }

    #[test]
    fn test_server_info_with_pid() {
        let info = ServerInfo::new(12521).with_pid(1234);
        assert_eq!(info.pid, Some(1234));
    }

    // Note: list_servers() actually probes the network,
    // so we don't test it in unit tests.
}
