#![no_std]
#![deny(unsafe_code)]
//! Rust adapters for the language-independent Kernel ABI.

extern crate alloc;

#[allow(unsafe_code)]
mod export;

pub use export::export;

#[allow(unsafe_code)]
mod import;

pub use import::import;

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
        // Our exporter supplies ABI v1; its code is part of this same image.
        unsafe { super::import(super::export(connection), Rc::new(())) }
    }
}
