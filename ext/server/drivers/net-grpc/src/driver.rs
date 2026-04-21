//! `GrpcServerDriverImpl` — tonic-backed implementation of
//! [`reovim_subsys_net::GrpcServerDriver`].

use {
    arc_swap::ArcSwapOption,
    async_trait::async_trait,
    reovim_subsys_net::{
        GrpcServerDriver, NetError, ShutdownSignal, TransportConfig, TransportKind,
    },
    std::{net::SocketAddr, sync::Arc},
    tokio::net::TcpListener,
    tokio_stream::wrappers::TcpListenerStream,
};

/// Driver that serves a prepared `tonic::transport::server::Router`.
///
/// Constructed via [`GrpcServerDriverImpl::new`]; the composition
/// root retains a clone of [`bind_handle`][GrpcServerDriver::bind_handle]
/// BEFORE calling [`serve`][GrpcServerDriver::serve] so the bound
/// address is readable while the serve future is running.
pub struct GrpcServerDriverImpl {
    bind: Arc<ArcSwapOption<SocketAddr>>,
}

impl GrpcServerDriverImpl {
    /// Create a driver with an empty bind-handle.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bind: Arc::new(ArcSwapOption::const_empty()),
        }
    }
}

impl Default for GrpcServerDriverImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GrpcServerDriver for GrpcServerDriverImpl {
    async fn serve(
        self: Box<Self>,
        config: TransportConfig,
        router: tonic::transport::server::Router,
        shutdown: Option<ShutdownSignal>,
    ) -> Result<(), NetError> {
        match config {
            TransportConfig::Stdio => Err(NetError::UnsupportedTransport(TransportKind::Stdio)),
            TransportConfig::Tcp { host, port } => {
                let addr = parse_tcp_addr(&host, port)?;
                let listener = TcpListener::bind(addr)
                    .await
                    .map_err(|e| NetError::BindFailed(e.to_string()))?;
                let bound = listener
                    .local_addr()
                    .map_err(|e| NetError::BindFailed(e.to_string()))?;
                self.bind.store(Some(Arc::new(bound)));
                let incoming = TcpListenerStream::new(listener);
                match shutdown {
                    Some(signal) => router
                        .serve_with_incoming_shutdown(incoming, signal)
                        .await
                        .map_err(NetError::from),
                    None => router
                        .serve_with_incoming(incoming)
                        .await
                        .map_err(NetError::from),
                }
            }
            #[cfg(unix)]
            TransportConfig::UnixSocket { path } => {
                use {tokio::net::UnixListener, tokio_stream::wrappers::UnixListenerStream};

                let listener =
                    UnixListener::bind(&path).map_err(|e| NetError::BindFailed(e.to_string()))?;
                let incoming = UnixListenerStream::new(listener);
                match shutdown {
                    Some(signal) => router
                        .serve_with_incoming_shutdown(incoming, signal)
                        .await
                        .map_err(NetError::from),
                    None => router
                        .serve_with_incoming(incoming)
                        .await
                        .map_err(NetError::from),
                }
            }
            #[cfg(not(unix))]
            TransportConfig::UnixSocket { .. } => {
                Err(NetError::UnsupportedTransport(TransportKind::UnixSocket))
            }
        }
    }

    fn bind_handle(&self) -> Arc<ArcSwapOption<SocketAddr>> {
        Arc::clone(&self.bind)
    }
}

fn parse_tcp_addr(host: &str, port: u16) -> Result<SocketAddr, NetError> {
    format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| NetError::InvalidAddress(e.to_string()))
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
