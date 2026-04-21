#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! gRPC server-driver implementation.
//!
//! Implements [`reovim_subsys_net::GrpcServerDriver`] for the
//! three transport kinds expressed by [`reovim_subsys_net::TransportConfig`]:
//!
//! - `Tcp` — bound via `tokio::net::TcpListener` and fed to
//!   `tonic::transport::server::Router::serve_with_incoming*`.
//! - `UnixSocket` — bound via `tokio::net::UnixListener` (unix only).
//! - `Stdio` — rejected with `NetError::UnsupportedTransport(Stdio)`
//!   since gRPC-over-stdio is not a tonic construct.
//!
//! See [`GrpcServerDriverImpl`] for the concrete type composition
//! roots instantiate.

mod driver;

pub use driver::GrpcServerDriverImpl;
