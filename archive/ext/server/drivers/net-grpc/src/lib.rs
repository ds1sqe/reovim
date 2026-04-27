#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! gRPC server-driver implementation (cdylib).
//!
//! Implements [`reovim_subsys_net::GrpcServerDriver`] over `tonic`.
//! Built as both `cdylib` and `rlib`: the cdylib variant is dlopened
//! at runtime by the host's `reovim-subsys-driver-loader` (#774 SP02);
//! the rlib variant supports in-process unit tests.
//!
//! The cdylib export `REOVIM_NET_GRPC_DRIVER_VTABLE` is emitted by
//! [`reovim_driver_macros::declare_net_grpc_driver!`]; see SP02
//! Phase 3 for the macro contract.

mod driver;

pub use driver::GrpcServerDriverImpl;

reovim_driver_macros::declare_net_grpc_driver!(GrpcServerDriverImpl);
