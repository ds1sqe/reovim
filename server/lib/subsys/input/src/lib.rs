#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input subsystem contracts for reovim.
//!
//! Linux equivalent: `drivers/input/` + `include/linux/input.h`
//!
//! This crate re-exports the opaque `InputEvent` envelope and frozen header
//! helpers from `reovim-input-codec` (the closed UAPI mechanism layer).
//!
//! It additionally provides:
//! - Generic keybinding lookup primitives (`LookupState<C>`, `LookupResult<C>`,
//!   `KeymapQuery<C>`, `LookupPolicy<C>`, `EagerLookupPolicy`).
//! - `InputCodecRegistry` trait and `DefaultInputCodecRegistry` implementation.

pub mod binding;
pub mod input_event;
pub mod lookup;
pub mod registry;

#[cfg(test)]
mod binding_tests;
#[cfg(test)]
mod input_event_tests;
#[cfg(test)]
mod lookup_tests;
#[cfg(test)]
mod registry_tests;

pub use binding::{BindingInfo, BindingLayer};
pub use input_event::{
    INPUT_HEADER_SIZE, InputEvent, InputFlags, InputPayloadError, input_context, input_flags,
    input_kind,
};
pub use lookup::{EagerLookupPolicy, KeymapQuery, LookupPolicy, LookupResult, LookupState};
pub use registry::{DefaultInputCodecRegistry, InputCodecRegistry};

// Re-export InputSequence from uapi so consumers don't need a direct dep on
// reovim-input-codec for the common sequence type.
pub use reovim_input_codec::{Codec, InputSequence};
