//! Client module loading, dependency resolution, and lifecycle management.
//!
//! The canonical implementation is now in `reovim-client-subsys-module::loader`.
//! This module is a thin re-export wrapper so downstream code that imports
//! `reovim_client_driver::loader::*` keeps compiling without path changes.
//!
//! TODO(#753): Remove when `clients/lib/driver/` is decommissioned in Phase F.

pub use reovim_client_subsys_module::loader::*;

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
