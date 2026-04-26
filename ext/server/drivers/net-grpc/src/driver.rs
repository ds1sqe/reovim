//! `GrpcServerDriverImpl` — tonic-backed implementation of
//! [`reovim_subsys_net::GrpcServerDriver`].
//!
//! Constructed via [`GrpcServerDriverImpl::construct`]; owns its
//! own `tokio::runtime::Runtime` (per SP02 Phase 1 §D — driver
//! creates its own runtime to avoid cross-cdylib TLS coupling). The
//! `serve` body builds a `tonic::transport::server::Router` from the
//! supplied [`reovim_subsys_net::ServiceDescriptor`]s, binds the
//! listener, signals bind-ready, then serves until `shutdown_fd`
//! becomes readable.

#![allow(unsafe_code)] // Drop fd from raw int for tokio AsyncFd

use {
    async_trait::async_trait,
    reovim_subsys_net::{
        GrpcServerDriver, NetError, ServiceDescriptor, TransportConfig,
        abi::{NetGrpcDriverProbe, ShutdownFd},
    },
    std::{
        convert::Infallible,
        os::fd::{FromRawFd, OwnedFd},
        sync::atomic::{AtomicU16, Ordering},
    },
    tokio::{
        io::unix::AsyncFd,
        net::TcpListener,
        runtime::{Builder, Handle, Runtime},
    },
    tokio_stream::wrappers::TcpListenerStream,
    tonic::{body::BoxBody, server::NamedService, transport::Server as TonicServer},
    tower::util::BoxCloneService,
};

/// Macro: generate one named-service proxy per stable proto identifier.
///
/// Each proxy wraps the inner `BoxCloneService` and impls
/// `tonic::server::NamedService` with the proto name as `const NAME`,
/// satisfying `tonic::transport::Server::add_service`'s type-level
/// `NamedService` bound (per SP02 Phase 1 §C).
macro_rules! impl_named_proxy {
    ($($wrapper:ident => $name:literal,)+) => {
        $(
            #[derive(Clone)]
            struct $wrapper {
                inner: BoxCloneService<
                    http::Request<BoxBody>,
                    http::Response<BoxBody>,
                    Infallible,
                >,
            }

            impl NamedService for $wrapper {
                const NAME: &'static str = $name;
            }

            impl tower::Service<http::Request<BoxBody>> for $wrapper {
                type Response = http::Response<BoxBody>;
                type Error = Infallible;
                type Future = <
                    BoxCloneService<
                        http::Request<BoxBody>,
                        http::Response<BoxBody>,
                        Infallible,
                    > as tower::Service<http::Request<BoxBody>>
                >::Future;

                fn poll_ready(
                    &mut self,
                    cx: &mut std::task::Context<'_>,
                ) -> std::task::Poll<Result<(), Self::Error>> {
                    self.inner.poll_ready(cx)
                }

                fn call(&mut self, req: http::Request<BoxBody>) -> Self::Future {
                    self.inner.call(req)
                }
            }
        )+
    };
}

impl_named_proxy!(
    BufferProxy => "reovim.v3.BufferService",
    EditorProxy => "reovim.v3.EditorService",
    InputProxy => "reovim.v3.InputService",
    ModuleProxy => "reovim.v3.ModuleService",
    StateProxy => "reovim.v3.StateService",
    ServerProxy => "reovim.v3.ServerService",
    NotificationProxy => "reovim.v3.NotificationService",
    PresenceProxy => "reovim.v3.PresenceService",
    ExtensionProxy => "reovim.v3.ExtensionService",
    CommandProxy => "reovim.v3.CommandService",
    DebugProxy => "reovim.v3.DebugService",
    ClientDebugProxy => "reovim.v3.ClientDebugService",
);

/// Driver value owning the tokio runtime that drives serve.
///
/// `rt` is wrapped in `Option` so that `Drop` can call
/// `Runtime::shutdown_background` without blocking in async contexts.
/// The plain `Runtime::drop` blocks waiting for workers, which
/// panics when the driver is dropped from inside another tokio
/// runtime (e.g. host-side tests using `#[tokio::test]`).
pub struct GrpcServerDriverImpl {
    rt: Option<Runtime>,
}

impl GrpcServerDriverImpl {
    /// Static probe metadata invoked pre-construct.
    #[must_use]
    pub fn probe() -> NetGrpcDriverProbe {
        NetGrpcDriverProbe::new("net_grpc", "reovim-driver-net-grpc")
    }

    /// Construct a driver instance, allocating its own multi-thread
    /// tokio runtime (SP02 Phase 1 §D).
    ///
    /// # Errors
    /// Propagates `tokio::runtime::Builder::build` errors as
    /// [`NetError::Io`].
    pub fn construct() -> Result<Self, NetError> {
        let rt = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| NetError::Io(format!("runtime: {e}")))?;
        Ok(Self { rt: Some(rt) })
    }

    /// Handle to the driver-owned runtime; used by the macro-emitted
    /// serve trampoline to `block_on` the async serve future.
    ///
    /// # Panics
    /// Panics if the runtime has already been shut down (only
    /// possible after Drop has run, which means the driver value
    /// itself is gone — so this can't actually be observed).
    #[must_use]
    pub fn runtime_handle(&self) -> Handle {
        self.rt
            .as_ref()
            .expect("runtime present (construct succeeded; Drop not yet run)")
            .handle()
            .clone()
    }
}

impl Drop for GrpcServerDriverImpl {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            // Background shutdown avoids blocking when the driver is
            // dropped inside an async context (e.g. host tests using
            // `#[tokio::test]`). Workers complete on their own
            // schedule.
            rt.shutdown_background();
        }
    }
}

#[async_trait]
impl GrpcServerDriver for GrpcServerDriverImpl {
    async fn serve(
        &self,
        config: TransportConfig,
        descriptors: Vec<ServiceDescriptor>,
        shutdown_fd: ShutdownFd,
        bind_ready_fd: ShutdownFd,
        port_writeback: &'static AtomicU16,
    ) -> Result<(), NetError> {
        let TransportConfig::Tcp { host, port, .. } = config else {
            return Err(NetError::Io(
                "GrpcServerDriverImpl currently supports TCP only; \
                 UnixSocket landing in SP02 Phase 4 follow-on"
                    .into(),
            ));
        };

        let addr_str = format!("{host}:{port}");
        let addr: std::net::SocketAddr = addr_str
            .parse()
            .map_err(|e: std::net::AddrParseError| NetError::Io(e.to_string()))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| NetError::BindFailed(e.to_string()))?;
        let bound = listener
            .local_addr()
            .map_err(|e| NetError::BindFailed(e.to_string()))?;

        port_writeback.store(bound.port(), Ordering::Release);
        signal_bind_ready(bind_ready_fd);

        let mut routes_builder = tonic::service::Routes::builder();
        for desc in descriptors {
            match desc.name {
                "reovim.v3.BufferService" => {
                    routes_builder.add_service(BufferProxy { inner: desc.inner });
                }
                "reovim.v3.EditorService" => {
                    routes_builder.add_service(EditorProxy { inner: desc.inner });
                }
                "reovim.v3.InputService" => {
                    routes_builder.add_service(InputProxy { inner: desc.inner });
                }
                "reovim.v3.ModuleService" => {
                    routes_builder.add_service(ModuleProxy { inner: desc.inner });
                }
                "reovim.v3.StateService" => {
                    routes_builder.add_service(StateProxy { inner: desc.inner });
                }
                "reovim.v3.ServerService" => {
                    routes_builder.add_service(ServerProxy { inner: desc.inner });
                }
                "reovim.v3.NotificationService" => {
                    routes_builder.add_service(NotificationProxy { inner: desc.inner });
                }
                "reovim.v3.PresenceService" => {
                    routes_builder.add_service(PresenceProxy { inner: desc.inner });
                }
                "reovim.v3.ExtensionService" => {
                    routes_builder.add_service(ExtensionProxy { inner: desc.inner });
                }
                "reovim.v3.CommandService" => {
                    routes_builder.add_service(CommandProxy { inner: desc.inner });
                }
                "reovim.v3.DebugService" => {
                    routes_builder.add_service(DebugProxy { inner: desc.inner });
                }
                "reovim.v3.ClientDebugService" => {
                    routes_builder.add_service(ClientDebugProxy { inner: desc.inner });
                }
                other => {
                    return Err(NetError::Io(format!("unknown service: {other}")));
                }
            }
        }

        let mut server_builder = TonicServer::builder();
        let router = server_builder.add_routes(routes_builder.routes());
        let incoming = TcpListenerStream::new(listener);
        let shutdown_future = shutdown_when_fd_readable(shutdown_fd);

        router
            .serve_with_incoming_shutdown(incoming, shutdown_future)
            .await
            .map_err(NetError::from)
    }
}

/// Convert a host-owned [`ShutdownFd`] into an awaitable readiness
/// future. The fd is *borrowed*: ownership stays with the host and is
/// not closed by the driver. Implemented by wrapping the fd in
/// `OwnedFd` to satisfy `AsyncFd`'s ownership requirement, then
/// `mem::forget`-ing the `OwnedFd` at the end so its destructor (which
/// would `close()` the fd) does not run.
async fn shutdown_when_fd_readable(fd: ShutdownFd) {
    if fd.0 < 0 {
        // Sentinel for tests / no-op shutdown signalling: pend forever.
        std::future::pending::<()>().await;
        return;
    }
    // SAFETY: we take ownership of the fd value just to construct
    // AsyncFd, but we never close it — see the mem::forget at end.
    let owned = unsafe { OwnedFd::from_raw_fd(fd.0) };
    let Ok(async_fd) = AsyncFd::new(owned) else {
        std::future::pending::<()>().await;
        return;
    };
    let _ = async_fd.readable().await;
    // Recover the OwnedFd and forget it — host owns the fd.
    let owned = async_fd.into_inner();
    std::mem::forget(owned);
}

/// Signal bind-ready by writing one byte to the host-owned fd.
fn signal_bind_ready(fd: ShutdownFd) {
    if fd.0 < 0 {
        return;
    }
    // SAFETY: write(2) on a host-owned fd; we don't close.
    let _ = unsafe { libc_write(fd.0, &[1u8]) };
}

/// Best-effort `write(2)`. Errors are ignored: the host's port-reporter
/// uses `bind_ready` as an advisory signal and falls back to polling
/// the `AtomicU16` if the fd write fails.
unsafe fn libc_write(fd: std::os::fd::RawFd, buf: &[u8]) -> isize {
    // SAFETY: caller guarantees `fd` is open for write and `buf` is
    // a valid slice. We use the raw libc::write to avoid pulling
    // tokio into this signal path.
    unsafe { libc::write(fd, buf.as_ptr().cast::<std::ffi::c_void>(), buf.len()) }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
