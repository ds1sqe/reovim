#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Pure shared input contracts for the server/session seam.
//!
//! This crate owns the driver-free key-sequence contract plus the shared
//! mode/keybinding/session-adjacent contracts that are still consumed across
//! server, subsys, and driver crates during Mission #753 Plan 10 Phase 2A.

mod binding_info;
mod key_sequence;
mod lookup;
mod mode_info;
mod transition;

pub use {
    binding_info::BindingInfo,
    key_sequence::{KeySequence, ToKeyToken},
    lookup::{
        BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult, KeyLookupState,
        KeymapQuery,
    },
    mode_info::ModeInfo,
    transition::{ModeTransition, PopResult, TransitionContext},
};
