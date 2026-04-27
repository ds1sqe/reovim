//! gRPC server-driver contract.
//!
//! Subsys-net exposes a single async lifecycle trait for the gRPC
//! transport. Drivers (under `ext/server/drivers/net-grpc/`) implement
//! `serve`; the host (`server/lib/server/`) hands them a list of
//! pre-built [`ServiceDescriptor`]s plus file-descriptor-shaped
//! shutdown / bind-ready signals plus a host-owned `AtomicU16` slot
//! for the bound port.
//!
//! The trait is FFI-routable: every cross-process type is `#[repr(C)]`
//! or a stable opaque pointer (see [`crate::abi`]). Cdylib drivers
//! export the canonical `REOVIM_NET_GRPC_DRIVER_VTABLE` symbol via
//! the `declare_net_grpc_driver!` macro.

use {
    crate::{ServiceDescriptor, TransportConfig, abi::ShutdownFd, error::NetError},
    std::sync::atomic::AtomicU16,
};

/// Async server-driver lifecycle for the gRPC transport.
///
/// The composition root constructs `descriptors` from its 12
/// `<X>ServiceImpl` instances, allocates a host-owned `AtomicU16` for
/// the bound port (`Box::leak(Box::new(AtomicU16::new(0)))`), and
/// creates two file descriptors via `eventfd(2)` (Linux) or
/// `pipe2(2)` (other Unix): one for shutdown, one for bind-ready
/// signalling.
///
/// ```text
/// host:                            driver:
///   build_service_descriptors()      |
///       │                            |
///       ▼                            |
///   Box::leak(AtomicU16)             |
///       │                            |
///       ▼                            |
///   driver.serve(config, descs,      |
///                shutdown_fd,        |
///                bind_ready_fd,      |
///                port_writeback)  ─► bind listener
///                                 ─► port_writeback.store(port)
///                                 ─► write 1u8 to bind_ready_fd
///                                 ─► serve until shutdown_fd readable
///                                    │
///       ◄──────────────────────────  Ok(()) / Err(NetError)
/// ```
///
/// Cdylib lifecycle (cdylib loader path):
/// the host's `LoadedNetGrpc` wrapper translates each
/// `ServiceDescriptor` into an `FfiServiceDescriptor` (handle +
/// dispatch trampoline + destroy callback) and calls into the
/// driver's vtable `serve` slot. The driver builds the tonic Router
/// from typed wrapper types (one per stable `reovim.v3.<X>Service`
/// name; see SP02 Phase 1 §C) and serves it.
#[async_trait::async_trait]
pub trait GrpcServerDriver: Send + Sync + 'static {
    /// Start serving the configured transport with the supplied
    /// service descriptors.
    ///
    /// Borrows the driver — the host owns the `Box<dyn
    /// GrpcServerDriver>` and is responsible for dropping it after
    /// `serve` returns. (Borrow rather than consume because the FFI
    /// trampoline path needs to call `destroy` on the driver instance
    /// after `serve` completes; consuming `Box<Self>` here would
    /// risk double-free.)
    ///
    /// Returns when `shutdown_fd` becomes readable or when a fatal
    /// transport error occurs.
    ///
    /// # Errors
    /// - [`NetError::UnsupportedTransport`] when `config` asks for a
    ///   transport the driver cannot serve (e.g. `Stdio` for gRPC).
    /// - [`NetError::BindFailed`] on listener bind failure.
    /// - [`NetError::ServeFailed`] on tonic serve failure.
    async fn serve(
        &self,
        config: TransportConfig,
        descriptors: Vec<ServiceDescriptor>,
        shutdown_fd: ShutdownFd,
        bind_ready_fd: ShutdownFd,
        port_writeback: &'static AtomicU16,
    ) -> Result<(), NetError>;
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
