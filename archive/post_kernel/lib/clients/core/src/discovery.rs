//! Server discovery for finding running reovim instances.
//!
//! Scans default ports (12521-12530) to find running servers.

use std::{net::TcpStream, time::Duration};

/// Default server port.
///
/// Port 12521 is reserved for the manager daemon.
/// Server instances start at 12522.
pub const DEFAULT_PORT: u16 = 12522;

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

/// List running reovim servers.
///
/// Discovery strategy:
/// 1. Scan known ports (12522-12531) for any listening servers
/// 2. Scan /proc for reovim processes listening on any port (Linux only)
///
/// Results are deduplicated by port.
#[must_use]
pub fn list_servers() -> Vec<ServerInfo> {
    use std::collections::HashSet;

    let mut servers = Vec::new();
    let mut seen_ports = HashSet::new();

    // Phase 1: Scan known port range (fast, works on all platforms)
    for port in DEFAULT_PORT..DEFAULT_PORT + PORT_FALLBACK_COUNT {
        if is_port_open(DEFAULT_HOST, port) {
            let mut info = ServerInfo::new(port);

            // Try to get PID from /proc on Linux
            #[cfg(target_os = "linux")]
            if let Some(pid) = find_pid_for_port(port) {
                info = info.with_pid(pid);
            }

            seen_ports.insert(port);
            servers.push(info);
        }
    }

    // Phase 2: Scan /proc for reovim processes on any port (Linux only)
    #[cfg(target_os = "linux")]
    for (port, pid) in find_reovim_listening_ports() {
        if !seen_ports.contains(&port) && is_port_open(DEFAULT_HOST, port) {
            servers.push(ServerInfo::new(port).with_pid(pid));
            seen_ports.insert(port);
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

/// Find all reovim processes and their listening TCP ports.
///
/// Scans /proc to find processes named "reovim" that are listening on TCP sockets.
/// Returns a vector of (port, pid) tuples.
///
/// This enables discovery of servers running on non-standard ports (e.g., `--tcp 0`
/// which assigns a random high port).
#[cfg(target_os = "linux")]
fn find_reovim_listening_ports() -> Vec<(u16, u32)> {
    use std::{collections::HashMap, fs};

    let mut results = Vec::new();

    // Step 1: Build map of socket inode -> port from /proc/net/tcp
    // Only include sockets in LISTEN state (0A)
    let mut inode_to_port: HashMap<u64, u16> = HashMap::new();

    if let Ok(tcp_content) = fs::read_to_string("/proc/net/tcp") {
        for line in tcp_content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }

            // Check state - 0A = LISTEN
            if parts[3] != "0A" {
                continue;
            }

            // Parse port from local_address (format: IP:PORT in hex)
            let local_addr = parts[1];
            if let Some(port_hex) = local_addr.split(':').nth(1)
                && let Ok(port) = u16::from_str_radix(port_hex, 16)
            {
                // Get inode (parts[9])
                if let Ok(inode) = parts[9].parse::<u64>() {
                    inode_to_port.insert(inode, port);
                }
            }
        }
    }

    if inode_to_port.is_empty() {
        return results;
    }

    // Step 2: Scan /proc for reovim processes
    let Ok(proc_dir) = fs::read_dir("/proc") else {
        return results;
    };

    for entry in proc_dir.flatten() {
        let pid_str = entry.file_name().to_string_lossy().to_string();
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        // Check if this is a reovim process
        let comm_path = format!("/proc/{pid}/comm");
        let Ok(comm) = fs::read_to_string(&comm_path) else {
            continue;
        };
        let comm = comm.trim();

        // Match "reovim" or "reovim-server" etc.
        if !comm.starts_with("reovim") {
            continue;
        }

        // Step 3: Find listening sockets for this process
        let fd_path = format!("/proc/{pid}/fd");
        let Ok(fd_dir) = fs::read_dir(&fd_path) else {
            continue;
        };

        for fd_entry in fd_dir.flatten() {
            let Ok(link) = fs::read_link(fd_entry.path()) else {
                continue;
            };

            let link_str = link.to_string_lossy();

            // Check if this is a socket
            if !link_str.starts_with("socket:[") {
                continue;
            }

            // Extract inode from "socket:[12345]"
            let inode_str = link_str
                .trim_start_matches("socket:[")
                .trim_end_matches(']');
            let Ok(inode) = inode_str.parse::<u64>() else {
                continue;
            };

            // Check if this socket is in our listening ports map
            if let Some(&port) = inode_to_port.get(&inode) {
                results.push((port, pid));
            }
        }
    }

    results
}

/// Auto-discover a server and return connection config.
///
/// # Errors
///
/// Returns error if no servers found or multiple servers require explicit selection.
pub fn auto_discover() -> Result<crate::connection::ConnectionConfig, String> {
    let servers = list_servers();

    match servers.len() {
        0 => Err("No running reovim servers found".to_string()),
        1 => Ok(crate::connection::ConnectionConfig::tcp(&servers[0].host, servers[0].port)),
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
