//! Unix domain socket implementation for local transport.
//!
//! Linux equivalent: `net/unix/`
//!
//! Provides efficient IPC using Unix domain sockets. These offer:
//! - Zero-copy for small messages
//! - No network stack overhead
//! - File-based permissions
//!
//! # Usage
//!
//! ```ignore
//! use reovim_arch::unix::local::{UnixLocalListener, UnixLocalStream};
//! use std::path::Path;
//!
//! // Server side
//! let listener = UnixLocalListener::bind(Path::new("/tmp/reovim.sock")).await?;
//! let (stream, _) = listener.accept().await?;
//!
//! // Client side
//! let stream = UnixLocalStream::connect(Path::new("/tmp/reovim.sock")).await?;
//! ```

use {
    std::{io, path::Path},
    tokio::net::{UnixListener, UnixStream},
};

/// Unix domain socket listener for local transport.
///
/// Wraps `tokio::net::UnixListener` with automatic cleanup of stale sockets.
pub struct UnixLocalListener {
    listener: UnixListener,
    socket_path: std::path::PathBuf,
}

impl UnixLocalListener {
    /// Bind to a Unix socket path.
    ///
    /// If a stale socket file exists at the path, it will be removed before binding.
    ///
    /// # Arguments
    ///
    /// * `path` - Path for the Unix socket file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parent directory doesn't exist and can't be created
    /// - Socket file exists and is in use by another process
    /// - Permission denied
    pub async fn bind(path: &Path) -> io::Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try to remove stale socket file
        if path.exists() {
            // Try to connect to see if it's in use
            match UnixStream::connect(path).await {
                Ok(_) => {
                    // Socket is in use by another process
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        format!("Socket {} is already in use", path.display()),
                    ));
                }
                Err(_) => {
                    // Socket is stale, remove it
                    std::fs::remove_file(path)?;
                }
            }
        }

        let listener = UnixListener::bind(path)?;
        Ok(Self {
            listener,
            socket_path: path.to_path_buf(),
        })
    }

    /// Accept a new connection.
    ///
    /// Returns a stream for the connected client.
    ///
    /// # Errors
    ///
    /// Returns an error if the accept operation fails.
    pub async fn accept(&self) -> io::Result<(UnixLocalStream, tokio::net::unix::SocketAddr)> {
        let (stream, addr) = self.listener.accept().await?;
        Ok((UnixLocalStream { stream }, addr))
    }

    /// Get the socket path.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn path(&self) -> &Path {
        &self.socket_path
    }
}

impl Drop for UnixLocalListener {
    fn drop(&mut self) {
        // Clean up socket file on drop
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Unix domain socket stream for local transport.
///
/// Wraps `tokio::net::UnixStream` for async I/O.
pub struct UnixLocalStream {
    stream: UnixStream,
}

impl UnixLocalStream {
    /// Connect to a Unix socket.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Unix socket file
    ///
    /// # Errors
    ///
    /// Returns an error if the socket doesn't exist or connection is refused.
    pub async fn connect(path: &Path) -> io::Result<Self> {
        let stream = UnixStream::connect(path).await?;
        Ok(Self { stream })
    }

    /// Get a reference to the inner tokio stream.
    #[must_use]
    pub const fn inner(&self) -> &UnixStream {
        &self.stream
    }

    /// Get a mutable reference to the inner tokio stream.
    pub const fn inner_mut(&mut self) -> &mut UnixStream {
        &mut self.stream
    }

    /// Convert into the inner tokio stream.
    #[must_use]
    pub fn into_inner(self) -> UnixStream {
        self.stream
    }

    /// Split the stream into read and write halves.
    pub fn into_split(self) -> (tokio::net::unix::OwnedReadHalf, tokio::net::unix::OwnedWriteHalf) {
        self.stream.into_split()
    }
}

/// Check if a process with the given PID exists.
///
/// Uses the `kill(pid, 0)` trick to check process existence without sending a signal.
#[must_use]
pub fn process_exists(pid: u32) -> bool {
    // kill(pid, 0) checks if we can send a signal to the process
    // Returns Ok if process exists and we have permission
    // Returns Err(ESRCH) if process doesn't exist
    // Returns Err(EPERM) if process exists but we don't have permission (still exists!)
    let result = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output();

    match result {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unix_socket_bind_accept() {
        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join(format!("reovim-test-{}.sock", std::process::id()));

        // Clean up any leftover socket
        let _ = std::fs::remove_file(&socket_path);

        // Bind listener
        let listener = UnixLocalListener::bind(&socket_path).await.unwrap();
        assert!(socket_path.exists());

        // Connect from client (in separate task)
        let path = socket_path.clone();
        let client_handle =
            tokio::spawn(async move { UnixLocalStream::connect(&path).await.unwrap() });

        // Accept connection
        let (server_stream, _) = listener.accept().await.unwrap();
        let _client_stream = client_handle.await.unwrap();

        // Verify streams are connected
        assert!(server_stream.inner().peer_addr().is_ok());
    }

    #[tokio::test]
    async fn test_unix_socket_stale_cleanup() {
        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join(format!("reovim-test-stale-{}.sock", std::process::id()));

        // Create a stale socket file (just an empty file, not a real socket)
        std::fs::write(&socket_path, "").unwrap();
        assert!(socket_path.exists());

        // Binding should clean up the stale file and succeed
        let listener = UnixLocalListener::bind(&socket_path).await.unwrap();
        assert!(socket_path.exists());

        // Clean up
        drop(listener);
        assert!(!socket_path.exists());
    }

    #[test]
    fn test_process_exists_current() {
        // Current process should exist
        let pid = std::process::id();
        assert!(process_exists(pid));
    }

    #[test]
    fn test_process_exists_nonexistent() {
        // PID 1 (init) always exists, but a very high PID likely doesn't
        // Use a PID that's very unlikely to exist
        assert!(!process_exists(u32::MAX - 1));
    }
}
