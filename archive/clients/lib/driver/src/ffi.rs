//! FFI-safe boundary types and vtables for dynamic client modules.
//!
//! These types are now canonical in `reovim-client-subsys-module::ffi`.
//! This module is a thin re-export wrapper so downstream code that imports
//! `reovim_client_driver::ffi::*` keeps compiling without path changes.
//!
//! TODO(#753): Remove when `clients/lib/driver/` is decommissioned in Phase F.

pub use reovim_client_subsys_module::ffi::*;

#[cfg(test)]
#[path = "ffi_tests.rs"]
mod tests;
