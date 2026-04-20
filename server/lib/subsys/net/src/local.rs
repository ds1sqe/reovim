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
//! use reovim_subsys_net::local::LocalAddr;
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "local_tests.rs"]
mod tests;
