//! Shape-only tests for `GrpcServerDriver`.
//!
//! The trait is object-safe and async; behavioural tests (bind, serve,
//! shutdown) live in the driver crate (`reovim-driver-net-grpc`)
//! where a real tonic server can be spun up.

use {
    super::GrpcServerDriver,
    crate::{NetError, ServiceDescriptor, TransportConfig, abi::ShutdownFd},
    std::sync::atomic::AtomicU16,
};

/// Minimal `GrpcServerDriver` impl that returns immediately. Purely
/// for trait-object-safety + signature smoke coverage.
struct NoopDriver;

#[async_trait::async_trait]
impl GrpcServerDriver for NoopDriver {
    async fn serve(
        &self,
        _config: TransportConfig,
        _descriptors: Vec<ServiceDescriptor>,
        _shutdown_fd: ShutdownFd,
        _bind_ready_fd: ShutdownFd,
        _port_writeback: &'static AtomicU16,
    ) -> Result<(), NetError> {
        Ok(())
    }
}

#[test]
fn trait_is_object_safe() {
    let _: Box<dyn GrpcServerDriver> = Box::new(NoopDriver);
}

#[tokio::test]
async fn noop_driver_serve_returns_ok() {
    let driver: Box<dyn GrpcServerDriver> = Box::new(NoopDriver);
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));
    let result = driver
        .serve(
            TransportConfig::tcp_localhost(0),
            Vec::new(),
            ShutdownFd(-1),
            ShutdownFd(-1),
            writeback,
        )
        .await;
    assert!(result.is_ok());
}
