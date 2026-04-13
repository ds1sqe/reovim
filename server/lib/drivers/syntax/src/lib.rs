#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Syntax highlighting driver for reovim — bridge layer.
//!
//! Core traits and types are in [`reovim_subsys_syntax`]. This crate provides
//! the domain-specific bridge function `text_event_to_syntax_edit` that converts
//! text-domain events to syntax driver inputs.

mod bridge;

// Bridge functions: convert text-domain events to syntax driver inputs
pub use bridge::{compute_end_position, text_event_to_syntax_edit};

// Re-export everything from subsys-syntax for backwards compatibility
pub use reovim_subsys_syntax::*;
