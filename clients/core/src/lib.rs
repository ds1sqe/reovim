#![no_std]
#![forbid(unsafe_code)]
//! The Client: one per link, using the tree with the same ops a Module
//! uses (Documentation/Design/02-client.md).
//!
//! Bytes go out and come back through a `Carrier` the app provides; the
//! Client owns the `Endpoint`, asks a Pending reply until it is ready,
//! and offers what every user of the tree needs above the ops: a path,
//! a whole read, a method by name. A `Plugin` is what turns keys into
//! effects inside a Client.

extern crate alloc;

mod client;
mod fault;
pub mod input;
pub mod log;
mod plugin;
pub mod value;

pub use client::{Carrier, Client, ROOT};
pub use fault::Fault;
pub use input::Key;
pub use plugin::{Edit, Effect, Frame, Plugin, Quit};
// The contract types a consumer of the Client names, so it depends on
// this crate only.
pub use reovim_kernel_interface::{
    Decl, Error, Event, Flags, Handle, Hash, Info, Kind, Method, Rd, Rights, Ty,
};
