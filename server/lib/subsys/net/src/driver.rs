//! gRPC server-driver contract.
//!
//! Subsys-net exposes a single async lifecycle trait for the gRPC
//! transport. Drivers (under `ext/server/drivers/net-grpc/`) implement
//! `serve` by consuming a prepared `tonic::transport::server::Router`
//! and driving it against a listener derived from the `TransportConfig`.
//!
//! The shape is deliberately minimal: construction + serve + a
//! shared bind-address handle for the composition root to read the
//! OS-assigned port back.

use {
    crate::{NetError, TransportConfig},
    arc_swap::ArcSwapOption,
    std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc},
};

/// Shutdown future type alias used by `GrpcServerDriver::serve`.
///
/// Pinned + boxed + `Send` so the trait is object-safe when the
/// shutdown source is heterogeneous (Ctrl-C, oneshot channel, etc.).
pub type ShutdownSignal = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Async server-driver lifecycle for the gRPC transport.
///
/// ```text
///           bind_handle() -> clone Arc<ArcSwapOption<SocketAddr>>
///               │
///               ▼
/// driver = Box::new(GrpcServerDriverImpl::new())
///               │
///               ▼
/// driver.serve(config, router, shutdown) ────► listener.bind / router.serve_with_incoming
///               ▲                                      │
///               │                                      ▼
/// bind_handle filled after bind                  Ok(()) / Err(NetError)
/// ```
///
/// Callers obtain the bind-handle BEFORE calling `serve` (serve
/// consumes the box). The driver internally writes the bound
/// `SocketAddr` into the handle once the listener is live and
/// before the first request is accepted.
#[async_trait::async_trait]
pub trait GrpcServerDriver: Send + Sync + 'static {
    /// Start serving `router` on a listener derived from `config`.
    ///
    /// Consumes the driver box — the serve future owns the driver
    /// for its lifetime. Returns when the shutdown future resolves
    /// (if provided) or when a fatal transport error occurs.
    ///
    /// # Errors
    /// - [`NetError::UnsupportedTransport`] when `config` asks for a
    ///   transport the driver cannot serve (e.g. `Stdio` for gRPC).
    /// - [`NetError::BindFailed`] on listener bind failure.
    /// - [`NetError::ServeFailed`] on tonic serve failure.
    async fn serve(
        self: Box<Self>,
        config: TransportConfig,
        router: tonic::transport::server::Router,
        shutdown: Option<ShutdownSignal>,
    ) -> Result<(), NetError>;

    /// Clone of the shared bind-address handle.
    ///
    /// The composition root calls this BEFORE `serve` to retain a
    /// handle; after serve begins, the driver writes the bound
    /// `SocketAddr` into the inner `ArcSwap`. `None` means the
    /// listener has not yet bound (pre-bind) or the driver does not
    /// publish an address (e.g. Unix socket path already known).
    fn bind_handle(&self) -> Arc<ArcSwapOption<SocketAddr>>;
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
