#![no_std]
#![forbid(unsafe_code)]
//! The Client: one per link, using the tree with the same ops a Module
//! uses (Documentation/Design/02-client.md).
//!
//! Bytes go out and come back through a `Carrier` the app provides; the
//! `Tree` owns the `Endpoint` and asks a Pending reply until it is
//! ready. The `Host` is one door over that tree and the Client's own
//! under `/self`, and what it hands out is `Calls`: a Plugin makes its
//! ops through it, on either side, and reaches the Client's own devices
//! as the cells of `/self/dev`.

extern crate alloc;

pub mod bin;
mod buffers;
mod client;
pub mod dev;
mod fault;
mod host;
pub mod log;

pub use bin::Rvm;
pub use client::{Carrier, Client, Link, ROOT, Tree};
pub use dev::{Clip, Table, Want};
pub use fault::Fault;
pub use host::{Host, HostCalls, LOCAL_BASE, SELF};
// The contract types a consumer of the Client names, so it depends on
// this crate only.
pub use reovim_kernel_interface::{
    Calls, Decl, Error, Event, Field, Flags, Handle, Hash, Info, Kind, Method,
    Rd, Rights, Ty,
};
