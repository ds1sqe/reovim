//! Transport selection for the dual-mode launcher.
//!
//! The launcher's `Cli` exposes `--transport <kind>` + the matching
//! address flags (`--uds-path`, `--tcp-addr`, `--external-grpc`). This
//! module resolves those flags into a validated [`TransportChoice`] —
//! the form that both the server's [`reovim_server::TransportMode`]
//! and the client-side connect string are derived from — and rejects
//! the launch-mode/transport combinations that cannot work (see
//! `03b-launcher-and-transport.md` §Transport matrix).
//!
//! Validation layering: clap parses the raw flags, this module applies
//! the semantic rules, and only a valid `TransportChoice` is handed
//! to `embedded::run_embedded` / `subprocess::run_subprocess`.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use clap::ValueEnum;

use reovim_server::TransportMode;

/// CLI spelling of the transport kind, as parsed by clap's `ValueEnum`.
///
/// This is the raw user-visible form (`--transport inproc|pipe|uds|tcp`).
/// [`TransportChoice::resolve`] lifts one of these + the matching
/// address flags into the validated [`TransportChoice`] that the rest
/// of the launcher consumes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum TransportKind {
    /// In-process tonic-over-duplex transport (embedded only).
    Inproc,
    /// gRPC over OS pipe (launcher stdin/stdout for embedded, child
    /// stdio for subprocess).
    Pipe,
    /// gRPC over Unix-domain socket (Unix only).
    Uds,
    /// gRPC over TCP.
    Tcp,
}

/// Launch topology the launcher is running under.
///
/// Drives transport validation: [`TransportKind::Inproc`] is only
/// valid under [`LaunchMode::Embedded`]; [`LaunchMode::ExternalGrpc`]
/// requires TCP (the external server already owns its own transport
/// selector, and the launcher's role is reduced to handing the TUI a
/// connect string).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    /// Default `reovim` invocation: server + client in one process.
    Embedded,
    /// `--subprocess`: server and client are spawned as separate bins.
    Subprocess,
    /// `--external-grpc`: server runs elsewhere; launcher spawns only the client.
    ExternalGrpc,
}

/// Validated transport choice carrying any resolved address.
///
/// Produced by [`TransportChoice::resolve`] from the raw clap flags,
/// consumed by:
/// - [`TransportChoice::to_server_mode`] — selects the server-side
///   [`TransportMode`] arm when the launcher boots an embedded server.
/// - [`TransportChoice::to_client_connect_string`] — shapes the
///   client-side `--grpc` address (or equivalent) when the launcher
///   boots an embedded client that speaks to an OS-crossing transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportChoice {
    /// In-process duplex stream. No address.
    Inproc,
    /// OS-pipe. No address (launcher wires stdio).
    Pipe,
    /// Unix-domain socket at `path`.
    Uds {
        /// Filesystem path of the Unix socket.
        path: PathBuf,
    },
    /// TCP at `addr`.
    Tcp {
        /// Bound/listener socket address.
        addr: SocketAddr,
    },
}

/// Error produced when the raw CLI flags don't combine into a valid
/// [`TransportChoice`].
#[derive(Debug, PartialEq, Eq)]
pub enum TransportError {
    /// `--transport inproc --subprocess` — inproc has no OS-crossing
    /// wire, so it cannot cross a process boundary.
    InprocRequiresEmbedded,
    /// `--external-grpc` selected alongside `--transport {inproc,pipe,uds}`
    /// — an external gRPC server is always reached over TCP.
    ExternalGrpcRequiresTcp,
    /// `--transport uds` was requested on a platform without Unix-domain
    /// socket support. Caller should fall back to `--transport tcp`.
    UdsUnsupportedOnPlatform,
    /// `--transport tcp` was chosen without `--tcp-addr` and no sensible
    /// default could be derived.
    TcpAddrParse(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InprocRequiresEmbedded => f.write_str(
                "--transport inproc has no OS-crossing wire; it is only \
                 valid without --subprocess / --external-grpc",
            ),
            Self::ExternalGrpcRequiresTcp => f.write_str(
                "--external-grpc implies a remote gRPC server; only \
                 --transport tcp can reach it",
            ),
            Self::UdsUnsupportedOnPlatform => f.write_str(
                "--transport uds is not available on this platform; \
                 use --transport tcp",
            ),
            Self::TcpAddrParse(detail) => {
                write!(f, "--tcp-addr is not a valid host:port address: {detail}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

impl From<TransportError> for std::io::Error {
    fn from(err: TransportError) -> Self {
        Self::new(std::io::ErrorKind::InvalidInput, err.to_string())
    }
}

/// Default UDS path for a launcher spawning an embedded or subprocess
/// server (`$XDG_RUNTIME_DIR/reovim-<pid>.sock`, falling back to `/tmp`).
#[must_use]
pub fn default_uds_path() -> PathBuf {
    let runtime_dir =
        std::env::var_os("XDG_RUNTIME_DIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    runtime_dir.join(format!("reovim-{}.sock", std::process::id()))
}

/// Default TCP address for a launcher spawning an embedded or subprocess
/// server: loopback + OS-assigned port.
#[must_use]
pub fn default_tcp_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 0))
}

impl TransportChoice {
    /// Resolve the raw clap flags into a validated [`TransportChoice`].
    ///
    /// `kind` is the user-requested transport spelling. `uds_path` and
    /// `tcp_addr` are the optional address flags; defaults from
    /// [`default_uds_path`] / [`default_tcp_addr`] apply when the
    /// matching flag is unset. `mode` determines which combinations
    /// are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::InprocRequiresEmbedded`] for
    /// `(inproc, !Embedded)`,
    /// [`TransportError::ExternalGrpcRequiresTcp`] for
    /// `(!Tcp, ExternalGrpc)`,
    /// [`TransportError::UdsUnsupportedOnPlatform`] for `(Uds, !cfg(unix))`,
    /// or [`TransportError::TcpAddrParse`] when `--tcp-addr` is malformed.
    pub fn resolve(
        kind: TransportKind,
        mode: LaunchMode,
        uds_path: Option<&Path>,
        tcp_addr: Option<&str>,
    ) -> Result<Self, TransportError> {
        match (kind, mode) {
            (TransportKind::Inproc, LaunchMode::Subprocess | LaunchMode::ExternalGrpc) => {
                return Err(TransportError::InprocRequiresEmbedded);
            }
            (TransportKind::Pipe | TransportKind::Uds, LaunchMode::ExternalGrpc) => {
                return Err(TransportError::ExternalGrpcRequiresTcp);
            }
            _ => {}
        }

        match kind {
            TransportKind::Inproc => Ok(Self::Inproc),
            TransportKind::Pipe => Ok(Self::Pipe),
            TransportKind::Uds => {
                if cfg!(unix) {
                    let path = uds_path.map_or_else(default_uds_path, Path::to_path_buf);
                    Ok(Self::Uds { path })
                } else {
                    Err(TransportError::UdsUnsupportedOnPlatform)
                }
            }
            TransportKind::Tcp => {
                let addr = tcp_addr.map_or_else(
                    || Ok(default_tcp_addr()),
                    |s| {
                        s.parse::<SocketAddr>()
                            .map_err(|e| TransportError::TcpAddrParse(e.to_string()))
                    },
                )?;
                Ok(Self::Tcp { addr })
            }
        }
    }

    /// Map to the server-side [`TransportMode`] used in the
    /// [`reovim_server::ServerConfig`] built for the embedded server.
    ///
    /// [`TransportChoice::Inproc`] maps to [`TransportMode::Inproc`]
    /// (the launcher then calls [`reovim_server::Server::run_inproc`]
    /// directly, bypassing the [`TransportMode`] dispatch).
    #[must_use]
    pub fn to_server_mode(&self) -> TransportMode {
        match self {
            Self::Inproc => TransportMode::Inproc,
            Self::Pipe => TransportMode::Pipe,
            #[cfg(unix)]
            Self::Uds { path } => TransportMode::UnixSocket { path: path.clone() },
            #[cfg(not(unix))]
            Self::Uds { .. } => unreachable!("UDS rejected on non-unix in resolve"),
            Self::Tcp { addr } => TransportMode::Grpc { port: addr.port() },
        }
    }

    /// Shape the client-side `--grpc host:port` connect string.
    ///
    /// Returns `None` for [`TransportChoice::Inproc`] and
    /// [`TransportChoice::Pipe`] (those transports pipe a stream
    /// directly into the client and don't use a connect string).
    ///
    /// [`TransportChoice::Tcp`] returns `"host:port"`, unambiguous for
    /// the TUI's `--grpc` flag. [`TransportChoice::Uds`] returns the
    /// socket path verbatim; the client-side UDS connect currently
    /// consumes this via its own dedicated path (tracked under #769 —
    /// the UDS client path is completed in a later sub-phase).
    #[must_use]
    pub fn to_client_connect_string(&self) -> Option<String> {
        match self {
            Self::Inproc | Self::Pipe => None,
            Self::Uds { path } => Some(path.display().to_string()),
            Self::Tcp { addr } => Some(addr.to_string()),
        }
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
