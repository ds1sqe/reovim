//! Concrete lowering and read-only derived snapshots for two native families.
//!
//! Only public native state/schema and Domain meaning are dependencies;
//! Resource adapter storage and Kernel authority are never accessed here.

#![no_std]

extern crate alloc;

mod descriptor;
mod lowering;
mod snapshot;

pub use lowering::{HeadingRepresentation, KeyedRepresentation};
pub use snapshot::{HeadingSnapshot, KeyedSnapshot};
