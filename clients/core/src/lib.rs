#![no_std]
#![forbid(unsafe_code)]
//! The Client: one per link, using the tree with the same ops a Module
//! uses (Documentation/Design/02-client.md).
//!
//! Bytes go out and come back through a `Carrier` the app provides; the
//! `Tree` owns the `Endpoint`, asks a Pending reply until it is ready,
//! and offers what every user of the tree needs above the ops: a path,
//! a whole read, a method by name. The `Client` is that tree and the
//! watch cursor it drains for the `Plugin`s it holds, each of which
//! makes its own calls and reaches the Client itself by capability.

extern crate alloc;

mod caps;
mod client;
mod fault;
pub mod input;
pub mod log;
mod plugin;
pub mod value;

pub use caps::{Caps, MATCH, SELECTION, grid, keys, process};
pub use client::{Carrier, Client, ROOT, Tree, under};
pub use fault::Fault;
pub use input::Key;
pub use plugin::{Ctx, Plugin, Start};
// The contract types a consumer of the Client names, so it depends on
// this crate only.
pub use reovim_kernel_interface::{
    Decl, Error, Event, Flags, Handle, Hash, Info, Kind, Method, Rd, Rights, Ty,
};
