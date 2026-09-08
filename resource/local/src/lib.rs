//! Bounded native families and immutable in-memory Resource Access.
//!
//! Native states and schemas are public for concrete Domain/Representation
//! providers. Adapter storage and request ownership remain private. The
//! reference profile permits one open and one initial observation per Access;
//! Buffer edits do not publish changes back to this immutable source.

#![no_std]

extern crate alloc;

mod bytes;
mod keyed;
mod memory;
mod native;
pub mod profile;

pub use bytes::{ByteModel, ByteState};
pub use keyed::{Key, KeyedModel, KeyedState};
pub use memory::{MemoryResource, MemoryResourceError};

/// Maximum source payload admitted by the bounded reference profile.
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024;
