//! `BindingInfo` re-export from `reovim-subsys-input`.
//!
//! The canonical definition of `BindingInfo` now lives in
//! `reovim-subsys-input` (mechanism layer).  This module re-exports it so
//! existing internal code that imports from `crate::binding_info` continues
//! to work without change.
//!
//! `BindingLayer` is re-exported from `crate::lookup_contracts` instead to
//! avoid a duplicate re-export here.

pub use reovim_subsys_input::binding::BindingInfo;
