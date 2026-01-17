//! Windows Named Pipe implementation for local transport.
//!
//! Windows equivalent of Unix domain sockets using Named Pipes.
//!
//! # Architecture
//!
//! Named pipes on Windows work differently from Unix sockets:
//! - Pipe names are in the format `\\.\pipe\<name>`
//! - Server creates pipe instances, clients connect to them
//! - Each connection requires a new pipe instance
//!
//! # Usage
//!
//! ```ignore
//! use reovim_arch::windows::local::{WindowsLocalListener, WindowsLocalStream};
//!
//! // Server side
//! let listener = WindowsLocalListener::bind("reovim-myproject").await?;
//! let stream = listener.accept().await?;
//!
//! // Client side
//! let stream = WindowsLocalStream::connect("reovim-myproject").await?;
//! ```

use {
    std::io,
    tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
};

/// Windows Named Pipe listener for local transport.
///
/// Creates and manages named pipe instances for incoming connections.
pub struct WindowsLocalListener {
    pipe_name: String,
    server: Option<NamedPipeServer>,
}

impl WindowsLocalListener {
    /// Bind to a named pipe.
    ///
    /// # Arguments
    ///
    /// * `name` - Pipe name (will be prefixed with `\\.\pipe\`)
    ///
    /// # Errors
    ///
    /// Returns an error if the pipe already exists or permission denied.
    pub async fn bind(name: &str) -> io::Result<Self> {
        let pipe_name = format!(r"\\.\pipe\{name}");
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)?;

        Ok(Self {
            pipe_name,
            server: Some(server),
        })
    }

    /// Accept a new connection.
    ///
    /// Returns a stream for the connected client.
    pub async fn accept(&mut self) -> io::Result<WindowsLocalStream> {
        let server = self
            .server
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Listener already closed"))?;

        // Wait for client to connect
        server.connect().await?;

        // Create next server instance for subsequent connections
        let next_server = ServerOptions::new().create(&self.pipe_name)?;
        self.server = Some(next_server);

        Ok(WindowsLocalStream::Server(server))
    }

    /// Get the pipe name.
    #[must_use]
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }
}

/// Windows Named Pipe stream for local transport.
///
/// Can be either a server-side or client-side pipe.
pub enum WindowsLocalStream {
    /// Server-side pipe (from accept)
    Server(NamedPipeServer),
    /// Client-side pipe (from connect)
    Client(tokio::net::windows::named_pipe::NamedPipeClient),
}

impl WindowsLocalStream {
    /// Connect to a named pipe.
    ///
    /// # Arguments
    ///
    /// * `name` - Pipe name (will be prefixed with `\\.\pipe\`)
    ///
    /// # Errors
    ///
    /// Returns an error if the pipe doesn't exist or connection refused.
    pub async fn connect(name: &str) -> io::Result<Self> {
        let pipe_name = format!(r"\\.\pipe\{name}");
        let client = ClientOptions::new().open(&pipe_name)?;
        Ok(Self::Client(client))
    }
}

/// Check if a Windows process exists by PID.
///
/// Uses `OpenProcess` to check if the process is still running.
#[must_use]
pub fn process_exists(pid: u32) -> bool {
    use std::ptr::null_mut;

    // PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    // STILL_ACTIVE = 259
    const STILL_ACTIVE: u32 = 259;

    extern "system" {
        fn OpenProcess(
            desired_access: u32,
            inherit_handle: i32,
            process_id: u32,
        ) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(process: *mut std::ffi::c_void, exit_code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }

    // SAFETY: These are standard Windows API calls with valid parameters
    #[allow(unsafe_code)]
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }

        let mut exit_code: u32 = 0;
        let result = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);

        result != 0 && exit_code == STILL_ACTIVE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_exists_current() {
        // Current process should exist
        let pid = std::process::id();
        assert!(process_exists(pid));
    }

    #[test]
    fn test_process_exists_nonexistent() {
        // A very high PID likely doesn't exist
        assert!(!process_exists(u32::MAX - 1));
    }
}
