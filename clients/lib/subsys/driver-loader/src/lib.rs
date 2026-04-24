#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(unsafe_code)]
//! Client driver-loader subsystem.
//!
//! Safe Rust wrappers over runtime-loaded driver cdylibs. Each driver
//! category has its own `Loaded<Kind>` type that owns a
//! [`reovim_dylib_loader::Library`] handle, validates the exported
//! vtable's header fields via pure memory reads, and exposes the
//! driver's capabilities as safe trait objects or borrowed
//! sub-handles.

pub mod client_debug;
pub mod client_render;
pub mod error;
mod rc;
pub mod validation;

pub use {
    client_debug::{LoadedClientDebug, LoadedDebugObserver},
    client_render::{LoadedClientRender, LoadedRenderTarget},
    error::{LoadError, ScanEntryError, ValidationError},
};
