#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input codec crate — encode/decode well-known input types into `InputEvent` payloads.
//!
//! This crate lives outside subsys (in `shared/`) because it defines kind values
//! and body layouts. Subsys defines only the envelope and header format.
//!
//! # Kind Registry
//!
//! | Range | Owner | Purpose |
//! |-------|-------|---------|
//! | `0x0001-0x00FF` | This crate (in-repo) | Well-known: key, pointer, scroll |
//! | `0x0100-0xFFFF` | External crates | Anyone can claim a range |

pub mod key;
pub mod pointer;
pub mod scroll;

#[cfg(test)]
mod key_tests;
#[cfg(test)]
mod pointer_tests;
#[cfg(test)]
mod scroll_tests;
