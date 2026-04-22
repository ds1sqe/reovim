//! Client-side service registry for cross-module communication.
//!
//! The canonical implementation is now in `reovim-client-subsys-module::services`.
//! This module is a thin re-export wrapper so downstream code that imports
//! `reovim_client_driver::services::*` keeps compiling without path changes.
//!
//! TODO(#753): Remove when `clients/lib/driver/` is decommissioned in Phase F.

pub use reovim_client_subsys_module::services::*;

#[cfg(test)]
#[path = "services_tests.rs"]
mod tests;
