//! Client module trait re-exports and driver-local extensions.
//!
//! The core trait family (`ClientModule`, `ModuleContext`, etc.) now lives in
//! `reovim-client-subsys-module::traits`. This module re-exports them so
//! internal driver code and downstream ext modules that import
//! `reovim_client_driver::traits::*` keep compiling unchanged.
//!
//! `CellGridClientModule` is a TUI-specific extension that stays driver-local
//! because it is a rendering-model concern, not a subsys ABI concern.
//!
//! TODO(#753): Remove when `clients/lib/driver/` is decommissioned.

mod cell_grid;

// Re-export the full subsys trait surface.
pub use reovim_client_subsys_module::traits::*;

// Driver-local extension.
pub use cell_grid::CellGridClientModule;

#[cfg(test)]
mod tests;
