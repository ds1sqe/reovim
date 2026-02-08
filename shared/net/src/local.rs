//! Platform-agnostic local transport.
//!
//! Provides efficient IPC for same-machine communication:
//! - Unix: Unix domain sockets
//! - Windows: Named pipes
//!
//! # Architecture
//!
//! This module defines the platform-agnostic types and path generation.
//! The actual implementations live in `shared/arch/`:
//! - `shared/arch/src/unix/local.rs` - Unix socket implementation
//! - `shared/arch/src/windows/local.rs` - Named pipe implementation
//!
//! # Usage
//!
//! ```ignore
//! use reovim_driver_net::local::LocalAddr;
//!
//! // Get platform-appropriate address for an instance
//! let addr = LocalAddr::for_instance("myproject");
//!
//! // On Unix: /run/user/1000/reovim/myproject.sock
//! // On Windows: \\.\pipe\reovim-myproject
//! ```

use std::path::PathBuf;

/// Local transport address.
///
/// Platform-specific address for local IPC:
/// - Unix: Unix domain socket path
/// - Windows: Named pipe path
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalAddr {
    /// Unix socket path (Linux/macOS).
    ///
    /// Format: `$XDG_RUNTIME_DIR/reovim/<name>.sock`
    /// Fallback: `/tmp/reovim-<uid>/<name>.sock`
    #[cfg(unix)]
    UnixSocket(PathBuf),

    /// Named pipe path (Windows).
    ///
    /// Format: `\\.\pipe\reovim-<name>`
    #[cfg(windows)]
    NamedPipe(String),
}

impl LocalAddr {
    /// Create platform-appropriate local address for an instance.
    ///
    /// # Arguments
    ///
    /// * `name` - Instance name (e.g., "default", "myproject")
    ///
    /// # Returns
    ///
    /// Platform-specific local address.
    ///
    /// # Platform Behavior
    ///
    /// - **Unix**: Returns `UnixSocket` with path in `$XDG_RUNTIME_DIR/reovim/`
    ///   or fallback to `/tmp/reovim-<uid>/`
    /// - **Windows**: Returns `NamedPipe` with name `\\.\pipe\reovim-<name>`
    #[must_use]
    pub fn for_instance(name: &str) -> Self {
        #[cfg(unix)]
        {
            Self::UnixSocket(Self::socket_path_for_instance(name))
        }

        #[cfg(windows)]
        {
            Self::NamedPipe(format!(r"\\.\pipe\reovim-{name}"))
        }
    }

    /// Get the socket directory path.
    ///
    /// # Platform Behavior
    ///
    /// - **Unix**: `$XDG_RUNTIME_DIR/reovim/` or `/tmp/reovim-<uid>/`
    /// - **Windows**: Not applicable (named pipes don't use directories)
    #[cfg(unix)]
    #[must_use]
    pub fn socket_dir() -> PathBuf {
        use reovim_arch::dirs::runtime_dir;

        runtime_dir().map_or_else(
            || {
                // Fallback for systems without XDG_RUNTIME_DIR (e.g., macOS)
                // Use username to avoid conflicts between users
                let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
                PathBuf::from(format!("/tmp/reovim-{user}"))
            },
            |d| d.join("reovim"),
        )
    }

    /// Get the full socket path for an instance.
    #[cfg(unix)]
    #[must_use]
    pub fn socket_path_for_instance(name: &str) -> PathBuf {
        Self::socket_dir().join(format!("{name}.sock"))
    }

    /// Get the pipe name for an instance (Windows only).
    #[cfg(windows)]
    #[must_use]
    pub fn pipe_name(&self) -> &str {
        match self {
            Self::NamedPipe(name) => name,
        }
    }

    /// Get the socket path (Unix only).
    #[cfg(unix)]
    #[must_use]
    pub fn socket_path(&self) -> &std::path::Path {
        match self {
            Self::UnixSocket(path) => path,
        }
    }
}

impl std::fmt::Display for LocalAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(unix)]
        {
            match self {
                Self::UnixSocket(path) => write!(f, "{}", path.display()),
            }
        }

        #[cfg(windows)]
        {
            match self {
                Self::NamedPipe(name) => write!(f, "{name}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_local_addr_for_instance_unix() {
        let addr = LocalAddr::for_instance("myproject");
        match addr {
            LocalAddr::UnixSocket(path) => {
                assert!(path.to_string_lossy().contains("myproject.sock"));
                assert!(path.to_string_lossy().contains("reovim"));
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_socket_dir_exists_or_can_be_created() {
        let dir = LocalAddr::socket_dir();
        // Should be a valid path (though directory might not exist yet)
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    #[cfg(windows)]
    fn test_local_addr_for_instance_windows() {
        let addr = LocalAddr::for_instance("myproject");
        match addr {
            LocalAddr::NamedPipe(name) => {
                assert_eq!(name, r"\\.\pipe\reovim-myproject");
            }
        }
    }

    #[test]
    fn test_local_addr_display() {
        let addr = LocalAddr::for_instance("test");
        let s = addr.to_string();
        assert!(s.contains("test"));
    }

    #[test]
    #[cfg(unix)]
    fn test_socket_path_accessor() {
        let addr = LocalAddr::for_instance("accessor-test");
        let path = addr.socket_path();
        assert!(path.to_string_lossy().contains("accessor-test.sock"));
    }

    #[test]
    #[cfg(unix)]
    fn test_socket_path_for_instance() {
        let path = LocalAddr::socket_path_for_instance("my-inst");
        assert!(path.to_string_lossy().contains("my-inst.sock"));
        assert!(path.to_string_lossy().contains("reovim"));
    }

    #[test]
    fn test_local_addr_equality() {
        let a = LocalAddr::for_instance("same");
        let b = LocalAddr::for_instance("same");
        assert_eq!(a, b);

        let c = LocalAddr::for_instance("different");
        assert_ne!(a, c);
    }

    #[test]
    fn test_local_addr_clone() {
        let a = LocalAddr::for_instance("clone-test");
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_local_addr_debug() {
        let addr = LocalAddr::for_instance("debug-test");
        let debug_str = format!("{addr:?}");
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_local_addr_display_format() {
        let addr = LocalAddr::for_instance("display-test");
        let display = format!("{addr}");
        assert!(display.contains("display-test"));
    }

    #[test]
    #[cfg(unix)]
    fn test_socket_dir_contains_reovim() {
        let dir = LocalAddr::socket_dir();
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains("reovim"), "Socket dir should contain 'reovim': {dir_str}");
    }

    #[test]
    #[cfg(unix)]
    fn test_multiple_instances_different_paths() {
        let a = LocalAddr::for_instance("inst-a");
        let b = LocalAddr::for_instance("inst-b");
        assert_ne!(a, b);

        let path_a = a.socket_path();
        let path_b = b.socket_path();
        assert_ne!(path_a, path_b);
    }
}
