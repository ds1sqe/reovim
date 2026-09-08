//! Concrete meaning for the public local native families.
//!
//! This package consumes only `resource-local`'s public state/schema exports,
//! never `MemoryResource` internals. Domain validation reads actual working
//! state; it does not mutate it or choose Buffer/History authority.

#![no_std]

mod heading;
mod keyed;
pub mod profile;

pub use heading::{Heading, HeadingDomain, heading, replacement};
pub use keyed::{KeyedDomain, selected_key};
