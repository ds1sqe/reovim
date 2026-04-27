#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Network subsystem contracts for reovim.
//!
//! Linux equivalent: `net/`, `include/linux/net.h`.
//!
//! # Architecture
//!
//! Subsys-net defines the gRPC server-driver lifecycle contract and
//! its supporting types ([`TransportConfig`], [`NetError`],
//! [`PortAllocator`], [`ServiceDescriptor`]). Concrete drivers live
//! under `ext/server/drivers/net-*/` and implement [`GrpcServerDriver`];
//! the server crate consumes this trait through
//! `Box<dyn GrpcServerDriver>`. Cdylib drivers export the canonical
//! `REOVIM_NET_GRPC_DRIVER_VTABLE` symbol via the
//! `declare_net_grpc_driver!` macro under `uapi/driver-macros/`.
//!
//! ```text
//! server/lib/subsys/net/        <-- Contracts (this crate)
//!        ^
//!        |  implemented by
//!        |
//! ext/server/drivers/net-grpc/  <-- cdylib driver (#774 SP02)
//! ```
//!
//! # Components
//!
//! - [`GrpcServerDriver`] — async serve-lifecycle trait
//! - [`TransportConfig`] — where/how to bind
//! - [`TransportKind`] — tag used by error variants
//! - [`NetError`] — unified error type
//! - [`PortAllocator`] — transport-neutral port picker
//! - [`ServiceDescriptor`] — host-built tonic service handed to the
//!   driver
//! - [`abi`] — `#[repr(C)]` FFI types for cdylib loading
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

pub mod abi;
mod driver;
mod error;
mod service_descriptor;
mod traits;
pub mod transport;

pub use {
    driver::GrpcServerDriver,
    error::NetError,
    service_descriptor::ServiceDescriptor,
    traits::PortAllocator,
    transport::{TransportConfig, TransportKind},
};
