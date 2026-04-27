#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Domain coordination contracts for reovim.
//!
//! This crate defines the domain-neutral types that the server uses to store
//! and transport positions and cursors without knowing domain internals.
//!
//! # Architecture
//!
//! ```text
//! subsys-coordination (this crate)
//! ├── PositionHeader / CursorHeader   — [u8; 8] discriminants
//! ├── Position / Cursor traits        — dyn trait objects with codec pattern
//! ├── PositionCodec / CursorCodec     — decode wire bytes → trait objects
//! └── CoordinationRegistry            — driver enlistment + codec lookup
//! ```
//!
//! # Codec pattern
//!
//! Every position and cursor carries a fixed 8-byte header and opaque content
//! bytes. The header layout is:
//!
//! ```text
//! [0..4]  domain_id: u32   (assigned by server at driver enlistment)
//! [4..6]  inner_id: u16    (type variant within domain)
//! [6..8]  flags: u16       (domain-defined flags)
//! ```
//!
//! The server can compare, clone, encode, and display positions and cursors
//! without understanding the domain-specific content bytes.
//!
//! # Zero domain dependencies
//!
//! This crate has zero external dependencies — it defines pure Rust contracts.
//! Domain-specific implementations live in driver crates (e.g.,
//! `driver-text-session` implements `TextPosition` and `TextCursor`).

mod codec;
mod cursor;
mod position;
pub mod projection;
#[cfg(test)]
mod projection_tests;
pub mod register;
#[cfg(test)]
mod register_tests;
mod registry;

pub use {
    codec::{CursorCodec, PositionCodec},
    cursor::{Cursor, CursorHeader},
    position::{Position, PositionHeader},
    projection::{DomainId, Projection, ProjectionDelivery, ProjectionTag},
    register::{RegisterKey, RegisterName},
    registry::{CoordinationRegistry, EnlistError},
};
