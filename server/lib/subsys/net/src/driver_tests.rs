//! Shape-only tests for `GrpcServerDriver`.
//!
//! The trait is object-safe and async; behavioral tests (bind, serve,
//! shutdown) live in the driver crate (`reovim-driver-net-grpc`,
//! Plan 15 N.2) where a real tonic server can be spun up.

use {
    super::{GrpcServerDriver, ShutdownSignal},
    crate::{NetError, TransportConfig},
    arc_swap::ArcSwapOption,
    std::{net::SocketAddr, sync::Arc},
};

// A minimal GrpcServerDriver impl that never actually serves.
// Purely for trait-object-safety + signature smoke coverage.
struct NoopDriver {
    bind: Arc<ArcSwapOption<SocketAddr>>,
}

#[async_trait::async_trait]
impl GrpcServerDriver for NoopDriver {
    async fn serve(
        self: Box<Self>,
        _config: TransportConfig,
        _router: tonic::transport::server::Router,
        _shutdown: Option<ShutdownSignal>,
    ) -> Result<(), NetError> {
        Ok(())
    }

    fn bind_handle(&self) -> Arc<ArcSwapOption<SocketAddr>> {
        Arc::clone(&self.bind)
    }
}

#[test]
fn trait_is_object_safe() {
    let _: Box<dyn GrpcServerDriver> = Box::new(NoopDriver {
        bind: Arc::new(ArcSwapOption::const_empty()),
    });
}

#[test]
fn bind_handle_starts_empty() {
    let d = NoopDriver {
        bind: Arc::new(ArcSwapOption::const_empty()),
    };
    assert!(d.bind_handle().load().is_none());
}

#[test]
fn bind_handle_clones_share_state() {
    let d = NoopDriver {
        bind: Arc::new(ArcSwapOption::const_empty()),
    };
    let h1 = d.bind_handle();
    let h2 = d.bind_handle();
    let addr: SocketAddr = "127.0.0.1:12521".parse().unwrap();
    h1.store(Some(Arc::new(addr)));
    assert_eq!(h2.load().as_deref().copied(), Some(addr));
}

#[test]
fn shutdown_signal_is_pin_box_dyn_future() {
    // Construction check: a tokio oneshot wrapped in a Box<Future>
    // satisfies the ShutdownSignal type alias.
    let (_tx, rx) = tokio::sync::oneshot::channel::<()>();
    let _signal: ShutdownSignal = Box::pin(async move {
        let _ = rx.await;
    });
}
