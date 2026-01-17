//! Instance information for registry.
//!
//! Defines the [`InstanceInfo`] struct that represents a running reovim server instance.
//! This information is persisted to the registry for discovery by clients.

use {
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
};

/// Information about a running reovim instance.
///
/// This struct is serialized to JSON and stored in the instance registry.
/// Clients use this information to discover and connect to running servers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceInfo {
    /// Instance name (e.g., "default", "project-foo").
    ///
    /// Must be a valid identifier: alphanumeric, hyphens, underscores.
    pub name: String,

    /// Process ID of the server.
    pub pid: u32,

    /// Transport address for connecting to this instance.
    pub transport: TransportInfo,

    /// Unix timestamp when the instance was started.
    pub started_at: u64,

    /// Working directory of the server (optional).
    pub cwd: Option<PathBuf>,
}

impl InstanceInfo {
    /// Create a new instance info.
    #[must_use]
    pub fn new(name: String, pid: u32, transport: TransportInfo) -> Self {
        Self {
            name,
            pid,
            transport,
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            cwd: std::env::current_dir().ok(),
        }
    }
}

/// Transport address information.
///
/// Describes how to connect to an instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "address")]
pub enum TransportInfo {
    /// TCP transport with host and port.
    Tcp {
        /// Host address (e.g., "127.0.0.1").
        host: String,
        /// Port number.
        port: u16,
    },

    /// Local transport (Unix socket or Windows named pipe).
    Local {
        /// Path to socket file or pipe name.
        path: String,
    },
}

impl TransportInfo {
    /// Create a TCP transport info.
    #[must_use]
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            host: host.into(),
            port,
        }
    }

    /// Create a local transport info.
    #[must_use]
    pub fn local(path: impl Into<String>) -> Self {
        Self::Local { path: path.into() }
    }

    /// Get a display string for the transport.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Tcp { host, port } => format!("{host}:{port}"),
            Self::Local { path } => path.clone(),
        }
    }
}

impl std::fmt::Display for TransportInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_info_serialization() {
        let info = InstanceInfo::new(
            "test-instance".to_string(),
            12345,
            TransportInfo::tcp("127.0.0.1", 12521),
        );

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: InstanceInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(info.name, deserialized.name);
        assert_eq!(info.pid, deserialized.pid);
        assert_eq!(info.transport, deserialized.transport);
    }

    #[test]
    fn test_transport_info_tcp() {
        let transport = TransportInfo::tcp("localhost", 9000);
        assert_eq!(transport.display(), "localhost:9000");
    }

    #[test]
    fn test_transport_info_local() {
        let transport = TransportInfo::local("/tmp/reovim.sock");
        assert_eq!(transport.display(), "/tmp/reovim.sock");
    }
}
