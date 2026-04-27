//! Transport configuration for the gRPC server.
//!
//! The subsys expresses transport intent as a plain data enum;
//! drivers translate the variant into a concrete listener.

use {crate::abi::FfiTransportConfig, std::path::PathBuf};

/// Lightweight tag for the transport variant, used by
/// `NetError::UnsupportedTransport` so error values don't have to
/// re-carry path/host/port payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// Communicate over stdin/stdout (no driver supports this for gRPC).
    Stdio,
    /// Unix domain socket.
    UnixSocket,
    /// TCP listener.
    Tcp,
}

/// Transport configuration for the gRPC server.
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Communicate over stdin/stdout.
    Stdio,
    /// Listen on a Unix domain socket at the given path.
    UnixSocket {
        /// Filesystem path for the socket.
        path: PathBuf,
        /// Layer the gRPC-Web translation on the served stack.
        /// Drivers gate this on the `grpc-web` Cargo feature; with the
        /// feature off, a `true` value here yields
        /// [`crate::NetError::UnsupportedTransport`].
        enable_grpc_web: bool,
    },
    /// Listen on TCP at the given host and port.
    Tcp {
        /// Host address (e.g., "127.0.0.1").
        host: String,
        /// Port number (0 to let the OS pick one).
        port: u16,
        /// Layer the gRPC-Web translation on the served stack. See
        /// [`Self::UnixSocket::enable_grpc_web`] for semantics.
        enable_grpc_web: bool,
    },
}

impl TransportConfig {
    /// Default port: 'r'*100 + 'e'*10 + 'o' = 12521.
    pub const DEFAULT_PORT: u16 = 12521;

    /// Last port in the default fallback range.
    pub const MAX_PORT: u16 = 12530;

    /// Build a Unix-socket config with gRPC-Web disabled.
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self::UnixSocket {
            path: path.into(),
            enable_grpc_web: false,
        }
    }

    /// Build a TCP config with gRPC-Web disabled.
    #[must_use]
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            host: host.into(),
            port,
            enable_grpc_web: false,
        }
    }

    /// Build a TCP config bound to `127.0.0.1` with gRPC-Web disabled.
    #[must_use]
    pub fn tcp_localhost(port: u16) -> Self {
        Self::tcp("127.0.0.1", port)
    }

    /// Builder-style enable-gRPC-Web setter.
    ///
    /// No-op for the [`Self::Stdio`] variant.
    #[must_use]
    pub fn with_grpc_web(self, enable: bool) -> Self {
        match self {
            Self::Stdio => Self::Stdio,
            Self::UnixSocket { path, .. } => Self::UnixSocket {
                path,
                enable_grpc_web: enable,
            },
            Self::Tcp { host, port, .. } => Self::Tcp {
                host,
                port,
                enable_grpc_web: enable,
            },
        }
    }

    /// Lightweight tag for use in error values.
    #[must_use]
    pub const fn kind(&self) -> TransportKind {
        match self {
            Self::Stdio => TransportKind::Stdio,
            Self::UnixSocket { .. } => TransportKind::UnixSocket,
            Self::Tcp { .. } => TransportKind::Tcp,
        }
    }

    /// gRPC-Web flag for the active variant. `Stdio` returns `false`.
    #[must_use]
    pub const fn enable_grpc_web(&self) -> bool {
        match self {
            Self::Stdio => false,
            Self::UnixSocket {
                enable_grpc_web, ..
            }
            | Self::Tcp {
                enable_grpc_web, ..
            } => *enable_grpc_web,
        }
    }

    /// Build a borrowed FFI image of this config.
    ///
    /// The returned `FfiTransportConfig` carries raw pointers into
    /// `self`'s string fields; the caller MUST keep `self` alive for
    /// the duration of the FFI call. Production callers (the cdylib
    /// loader's `LoadedNetGrpc::serve`) hold the source `TransportConfig`
    /// on the stack across the `vtable.serve` invocation.
    ///
    /// `kind` discriminates: 0 = `Tcp`, 1 = `UnixSocket`, 2 = `Stdio`
    /// (`Stdio` is rejected by the driver but produces a valid FFI
    /// image).
    #[must_use]
    pub fn into_ffi(&self) -> FfiTransportConfig {
        match self {
            Self::Stdio => FfiTransportConfig {
                kind: 2,
                host_ptr: std::ptr::null(),
                host_len: 0,
                port: 0,
                path_ptr: std::ptr::null(),
                path_len: 0,
                enable_grpc_web: 0,
            },
            Self::UnixSocket {
                path,
                enable_grpc_web,
            } => {
                let bytes = path.as_os_str().as_encoded_bytes();
                FfiTransportConfig {
                    kind: 1,
                    host_ptr: std::ptr::null(),
                    host_len: 0,
                    port: 0,
                    path_ptr: bytes.as_ptr().cast(),
                    path_len: bytes.len(),
                    enable_grpc_web: u8::from(*enable_grpc_web),
                }
            }
            Self::Tcp {
                host,
                port,
                enable_grpc_web,
            } => FfiTransportConfig {
                kind: 0,
                host_ptr: host.as_ptr().cast(),
                host_len: host.len(),
                port: *port,
                path_ptr: std::ptr::null(),
                path_len: 0,
                enable_grpc_web: u8::from(*enable_grpc_web),
            },
        }
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
