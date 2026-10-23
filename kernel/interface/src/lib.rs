#![no_std]
#![deny(unsafe_code)]
//! Kernel contract: Resource ids, flags, rights, replies, caller ops.
//!
//! Every Module and every wire endpoint shares these types. The Kernel
//! holds no domain word; see Documentation/Design/00-core.md.

extern crate alloc;

pub mod codec;
mod decl;
mod driver;
mod op;
mod render;
mod types;
pub mod value;
pub mod wire;

pub use decl::{Decl, Field, Method, Ty, fnv1a};
pub use driver::{
    Booted, Calls, Driver, Module, allows, check_call, part, spliced,
};
pub use op::{Op, Out};
pub use types::{
    Caller, DriverId, Error, Event, Flags, Handle, Hash, Info, Kind, ModuleId,
    Outcome, Rd, Reply, Rights, Token,
};
