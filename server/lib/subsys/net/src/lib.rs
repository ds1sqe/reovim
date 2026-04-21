#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Network subsystem contracts for reovim.
//!
//! Linux equivalent: `net/`, `include/linux/net.h`.
//!
//! # Architecture
//!
//! Subsys-net defines the gRPC server-driver lifecycle contract and
//! its supporting types (`TransportConfig`, `NetError`, `PortAllocator`).
//! Concrete drivers live under `ext/server/drivers/net-*/` and
//! implement [`GrpcServerDriver`]; the server crate consumes this
//! trait through `Box<dyn GrpcServerDriver>`.
//!
//! ```text
//! server/lib/subsys/net/        <-- Contracts (this crate)
//!        ^
//!        |  implemented by
//!        |
//! ext/server/drivers/net-grpc/  <-- Driver implementation (Plan 15 N.2)
//! ```
//!
//! # Components
//!
//! - [`GrpcServerDriver`] — async serve-lifecycle trait
//! - [`TransportConfig`] — where/how to bind
//! - [`TransportKind`] — tag used by error variants
//! - [`NetError`] — unified error type
//! - [`PortAllocator`] — transport-neutral port picker
//! - [`ShutdownSignal`] — boxed future type alias for `serve`'s
//!   shutdown parameter
//!
//! # Example
//!
//! ```no_run
//! use reovim_subsys_net::{TransportConfig, NetError};
//!
//! let config = TransportConfig::tcp_localhost(12521);
//! assert_eq!(config.kind(), reovim_subsys_net::TransportKind::Tcp);
//! # let _: Result<(), NetError> = Ok(());
//! ```

mod driver;
mod error;
mod traits;
pub mod transport;

pub use {
    driver::{GrpcServerDriver, ShutdownSignal},
    error::NetError,
    traits::PortAllocator,
    transport::{TransportConfig, TransportKind},
};
