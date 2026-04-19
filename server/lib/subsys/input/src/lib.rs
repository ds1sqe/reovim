#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input subsystem contracts for reovim.
//!
//! Linux equivalent: `drivers/input/` + `include/linux/input.h`
//!
//! This crate owns only the opaque `InputEvent` envelope and frozen header
//! helpers. Typed key/mouse vocabulary lives in `reovim-input-codec`.
pub mod input_event;
#[cfg(test)]
mod input_event_tests;

pub use input_event::{
    INPUT_HEADER_SIZE, InputEvent, InputFlags, InputPayloadError, input_context, input_flags,
    input_kind,
};
