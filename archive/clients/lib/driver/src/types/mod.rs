//! Type re-exports from `reovim-client-subsys-module`.
//!
//! Phase B (#753) moved all shared types to `reovim-client-subsys-module`.
//! This module is now a thin re-export wrapper so driver-internal code and
//! the 20 non-pilot ext modules keep compiling without path changes.
//!
//! TODO(#753): Removed in Phase F when `clients/lib/driver/` is decommissioned.

pub use reovim_client_subsys_module::types::*;

// `Color` was previously a direct `pub use reovim_arch::Color` in this module.
// It is now re-exported via `reovim_client_subsys_module::types` (which itself
// re-exports it from `reovim_arch`).

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
