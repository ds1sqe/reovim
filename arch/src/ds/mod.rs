//! Heap data structures owned by the platform floor.
//!
//! Because the `alloc` crate is forbidden (DAG6), these are the floor's
//! `Vec`/`String`/`HashMap`/`Arc` analogs: each owns raw memory from the arch
//! [`allocator`](crate::alloc) and is fallible where allocation can fail.
//! Crates above `arch/` consume these instead of `alloc`'s containers.
//!
//! - [`Seq`] — growable sequence (`Vec` analog).
//! - [`Bytes`]/[`Str`] — owned byte/UTF-8 strings.
//! - [`Map`] — open-addressing hash map (`HashMap` analog).
//! - [`Shared`] — atomic-refcount shared reference (`Arc` analog).
//! - [`Ring`] — bounded ring buffer with oldest-first eviction (LOG6 substrate).

mod bytes;
mod map;
mod ring;
mod seq;
mod shared;

pub use {
    bytes::{Bytes, BytesWriter, Str},
    map::{FxHasher, Map, MapIter},
    ring::{Ring, RingIter},
    seq::Seq,
    shared::Shared,
};

// L12 layout (#785 Phase 5): DS integration tests live in the sibling file
// `tests.rs`, declared here as a parent-declared sibling (all items under
// test are public).
#[cfg(feature = "selftest")]
mod tests;
