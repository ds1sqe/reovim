//! Viewport renderer — compatibility re-export.
//!
//! All types and functions are now canonical in `reovim-client-subsys-chrome`.
//! This module is a thin re-export shim preserved for the 20+ ext modules that
//! import `reovim_client_driver::viewport::*` until Phase F decommissions driver.
//!
//! TODO(#753): Phase F removes this file entirely.

pub use reovim_client_subsys_chrome::viewport::*;
