#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client render subsystem.
//!
//! Provides the `RenderTarget` trait and render pipeline contracts.

pub mod target;

pub use target::{RenderError, RenderTarget};
