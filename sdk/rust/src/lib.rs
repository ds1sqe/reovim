#![no_std]
#![deny(unsafe_code)]
//! Rust adapters for the language-independent Kernel ABI.

extern crate alloc;

#[allow(unsafe_code)]
mod export;

pub use export::{decode_request, export, export_reply, export_with_code};

#[allow(unsafe_code)]
mod provider;

#[allow(unsafe_code)]
mod import;

pub use import::{import, import_reply};

#[allow(unsafe_code)]
pub mod module;

/// Connects a local implementation through both C ABI adapters.
#[must_use]
pub fn local(
    connection: reovim_kernel_interface::Connection,
) -> reovim_kernel_interface::Connection {
    local::connect(connection)
}

#[allow(unsafe_code)]
mod local {
    use alloc::rc::Rc;
    use reovim_kernel_interface::Connection;

    pub fn connect(connection: Connection) -> Connection {
        // Our exporter supplies ABI 0.1.0; its code is part of this same image.
        unsafe { super::import(super::export(connection), Rc::new(())) }
    }
}
