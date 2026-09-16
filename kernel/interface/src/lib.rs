#![no_std]
#![deny(unsafe_code)]
//! Kernel contract: Resource ids, flags, rights, replies, caller ops.
//!
//! Every Module and every wire endpoint shares these types. The Kernel
//! holds no domain word; see Documentation/Design/00-core.md.

extern crate alloc;

mod decl;
mod driver;
mod op;
mod types;
pub mod value;

pub use decl::{Decl, Field, Method, Ty, fnv1a};
pub use driver::{Calls, Driver, Module};
pub use op::{Op, Out};
pub use types::{
    Caller, DriverId, Error, Event, Flags, Handle, Hash, Info, Kind, ModuleId,
    Outcome, Rd, Reply, Rights, Token,
};
