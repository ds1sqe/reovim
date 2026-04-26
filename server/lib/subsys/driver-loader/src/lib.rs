#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(unsafe_code)]
//! Server driver-loader subsystem.
//!
//! Safe Rust wrappers over runtime-loaded server-side driver cdylibs.
//! Mirrors the client-side `reovim-client-subsys-driver-loader` crate
//! but targets the server-tier driver families (currently only
//! `net-grpc`; additional families are added per #774 follow-on as
//! their trait redesigns land).
//!
//! Each family has its own `Loaded<Kind>` type that owns a
//! [`reovim_dylib_loader::Library`] handle, validates the exported
//! vtable's header fields via pure memory reads, and exposes the
//! driver's lifecycle as safe async methods.

pub mod buffer;
pub mod buffer_instance;
pub mod error;
pub mod net_grpc;
pub(crate) mod rc;
pub(crate) mod validation;

pub use {
    buffer::LoadedBuffer,
    error::{LoadError, ScanEntryError, ValidationError},
    net_grpc::LoadedNetGrpc,
};
