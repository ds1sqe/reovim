//! Render target — compatibility re-export.
//!
//! `RenderTarget` and `RenderError` are now canonical in
//! `reovim-client-subsys-render`. This module is a thin re-export shim
//! preserved for consumers that import `reovim_client_driver::render::*`
//! until Phase F decommissions driver.
//!
//! TODO(#753): Phase F removes this file entirely.

pub use reovim_client_subsys_render::{RenderError, RenderTarget};
